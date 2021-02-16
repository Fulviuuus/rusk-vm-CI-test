// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
//
// Copyright (c) DUSK NETWORK. All rights reserved.

mod contracts;

use rusk_vm::{
    Contract, ContractId, GasMeter, HostModule, NetworkState, VMError,
};

use dusk_bls12_381::BlsScalar;
use dusk_bytes::ParseHexStr;

use canonical::{ByteSource, Canon, Store};
use canonical_host::MemStore as MS;
use dusk_abi::{Query, ReturnValue};

use block_height::BlockHeight;
use counter::Counter;
use delegator::Delegator;
use fibonacci::Fibonacci;
use host_fn::HostFnTest;
use stack::Stack;
use verify_proof::ProofVerifier;

fn fibonacci_reference(n: u64) -> u64 {
    if n < 2 {
        n
    } else {
        fibonacci_reference(n - 1) + fibonacci_reference(n - 2)
    }
}

#[test]
fn counter() {
    let counter = Counter::new(99);

    let store = MS::new();

    let code = include_bytes!("contracts/counter/counter.wasm");

    let contract = Contract::new(counter, code.to_vec(), &store).unwrap();

    let mut network = NetworkState::<MS>::default();

    let contract_id = network.deploy(contract).unwrap();

    let mut gas = GasMeter::with_limit(1_000_000_000);

    assert_eq!(
        network
            .query::<_, i32>(contract_id, counter::READ_VALUE, &mut gas)
            .unwrap(),
        99
    );

    network
        .transact::<_, ()>(contract_id, counter::INCREMENT, &mut gas)
        .unwrap();

    assert_eq!(
        network
            .query::<_, i32>(contract_id, counter::READ_VALUE, &mut gas)
            .unwrap(),
        100
    );
}

#[test]
fn counter_trivial() {
    let counter = Counter::new(99);

    let store = MS::new();

    let code = include_bytes!("contracts/counter/counter.wasm");

    let contract = Contract::new(counter, code.to_vec(), &store).unwrap();

    let mut network = NetworkState::<MS>::default();

    let contract_id = network.deploy(contract).unwrap();

    let mut gas = GasMeter::with_limit(1_000_000_000);

    assert_eq!(
        network
            .query::<_, i32>(contract_id, counter::READ_VALUE, &mut gas)
            .unwrap(),
        99
    );
}

#[test]
fn delegated_call() {
    let counter = Counter::new(99);
    let delegator = Delegator;

    let store = MS::new();

    let mut network = NetworkState::<MS>::default();

    let counter_code = include_bytes!("contracts/counter/counter.wasm");
    let counter_contract =
        Contract::new(counter, counter_code.to_vec(), &store).unwrap();
    let counter_id = network.deploy(counter_contract).unwrap();

    let delegator_code = include_bytes!("contracts/delegator/delegator.wasm");
    let delegator_contract =
        Contract::new(delegator, delegator_code.to_vec(), &store).unwrap();
    let delegator_id = network.deploy(delegator_contract).unwrap();

    let mut gas = GasMeter::with_limit(1_000_000_000);

    // delegate query

    assert_eq!(
        network
            .query::<_, i32>(
                delegator_id,
                (delegator::DELEGATE_QUERY, counter_id, counter::READ_VALUE),
                &mut gas
            )
            .unwrap(),
        99
    );

    // delegate transaction

    network
        .transact::<_, ()>(
            delegator_id,
            (
                delegator::DELEGATE_TRANSACTION,
                counter_id,
                counter::INCREMENT,
            ),
            &mut gas,
        )
        .unwrap();

    // changed the value of counter

    assert_eq!(
        network
            .query::<_, i32>(counter_id, counter::READ_VALUE, &mut gas)
            .unwrap(),
        100
    );
}

#[test]
fn fibonacci() {
    let fib = Fibonacci;

    let store = MS::new();

    let code = include_bytes!("contracts/fibonacci/fibonacci.wasm");

    let contract = Contract::new(fib, code.to_vec(), &store).unwrap();

    let mut network = NetworkState::<MS>::default();

    let contract_id = network.deploy(contract).unwrap();

    let mut gas = GasMeter::with_limit(1_000_000_000);

    let n = 5;

    for i in 0..n {
        assert_eq!(
            network
                .query::<_, u64>(contract_id, (fibonacci::COMPUTE, i), &mut gas)
                .unwrap(),
            fibonacci_reference(i)
        );
    }
}

#[test]
fn stack() {
    let stack = Stack::new();

    let store = MS::new();

    let code = include_bytes!("contracts/stack/stack.wasm");

    let contract = Contract::new(stack, code.to_vec(), &store).unwrap();

    let mut network = NetworkState::<MS>::default();

    let contract_id = network.deploy(contract).unwrap();

    let mut gas = GasMeter::with_limit(1_000_000_000);

    let n: i32 = 64;

    for i in 0..n {
        network
            .transact::<_, ()>(contract_id, (stack::PUSH, i), &mut gas)
            .unwrap();
    }

    for i in 0..n {
        assert_eq!(
            network
                .query::<_, Option<i32>>(
                    contract_id,
                    (stack::PEEK, i),
                    &mut gas
                )
                .unwrap(),
            Some(i)
        );
    }

    for i in 0..n {
        let contract_state = network
            .get_contract_state::<Stack<MS>>(contract_id)
            .expect("A result")
            .expect("An option");

        assert_eq!(contract_state.peek(i), Some(i));
    }

    for i in 0..n {
        let i = n - i - 1;

        assert_eq!(
            network
                .transact::<_, Option<i32>>(contract_id, stack::POP, &mut gas)
                .unwrap(),
            Some(i)
        );
    }

    assert_eq!(
        network
            .transact::<_, Option<i32>>(contract_id, stack::POP, &mut gas)
            .unwrap(),
        None
    );
}

struct PoseidonModule<S> {
    store: S,
}

impl<S> PoseidonModule<S>
where
    S: Store,
{
    fn new(store: S) -> Self {
        PoseidonModule { store }
    }
}

impl<S> HostModule<S> for PoseidonModule<S>
where
    S: Store,
{
    fn execute(&self, query: Query) -> Result<ReturnValue, VMError<S>> {
        let mut source = ByteSource::new(query.as_bytes(), &self.store);

        let qid: u8 =
            Canon::<S>::read(&mut source).map_err(VMError::from_store_error)?;

        match qid {
            0 => {
                let scalars: Vec<BlsScalar> = Canon::<S>::read(&mut source)
                    .map_err(VMError::from_store_error)?;
                let ret = dusk_poseidon::sponge::hash(&scalars);

                ReturnValue::from_canon(&ret, &self.store)
                    .map_err(VMError::from_store_error)
            }
            _ => todo!(),
        }
    }
}

#[test]
fn hash_as_host_fn() {
    let test_inputs = [
        "bb67ed265bf1db490ded2e1ede55c0d14c55521509dc73f9c354e98ab76c9625",
        "7e74220084d75e10c89e9435d47bb5b8075991b2e29be3b84421dac3b1ee6007",
        "5ce5481a4d78cca03498f72761da1b9f1d2aa8fb300be39f0e4fe2534f9d4308",
    ];

    let test_inputs: Vec<BlsScalar> = test_inputs
        .iter()
        .map(|input| BlsScalar::from_hex_str(input).unwrap())
        .collect();

    let hash = HostFnTest::new();

    let store = MS::new();

    let code = include_bytes!("contracts/host_fn/host_fn.wasm");

    let contract = Contract::new(hash, code.to_vec(), &store).unwrap();

    let mut network = NetworkState::<MS>::default();

    let pos_mod = PoseidonModule::new(store.clone());

    network.register_host_module(ContractId::reserved(99), pos_mod);

    let contract_id = network.deploy(contract).unwrap();

    let mut gas = GasMeter::with_limit(1_000_000_000);

    assert_eq!(
        "0xe36f4ea9b858d5c85b02770823c7c5d8253c28787d17f283ca348b906dca8528",
        format!(
            "{:#x}",
            network
                .query::<_, BlsScalar>(
                    contract_id,
                    (host_fn::HASH, test_inputs),
                    &mut gas
                )
                .unwrap()
        )
    );
}

#[test]
fn block_height() {
    let bh = BlockHeight::new();

    let store = MS::new();

    let code = include_bytes!("contracts/block_height/block_height.wasm");

    let contract = Contract::new(bh, code.to_vec(), &store).unwrap();

    let mut network = NetworkState::<MS>::with_block_height(99);

    let contract_id = network.deploy(contract).unwrap();

    let mut gas = GasMeter::with_limit(1_000_000_000);

    assert_eq!(
        99,
        network
            .query::<_, u64>(contract_id, block_height::BLOCK_HEIGHT, &mut gas)
            .unwrap()
    )
}

#[test]
#[ignore]
fn proof_verifier() {
    use dusk_plonk::prelude::*;
    use transfer_circuits::ExecuteCircuit;

    let contract = ProofVerifier::new();

    let store = MS::new();

    let code = include_bytes!("contracts/verify_proof/verify_proof.wasm");

    let contract = Contract::new(contract, code.to_vec(), &store).unwrap();

    let mut network = NetworkState::<MS>::default();

    let contract_id = network.deploy(contract).unwrap();

    let mut gas = GasMeter::with_limit(1_000_000_000);

    // We store the current reference string here, so we can temporarily change
    // it for the test, and then change it back once finished.
    let old_crs_file_name = "temp_crs.bin";
    let old_crs = rusk_profile::get_common_reference_string();

    if let Ok(_) = old_crs {
        std::fs::File::create(old_crs_file_name).unwrap();
        std::fs::write(old_crs_file_name, old_crs.unwrap().as_slice()).unwrap();
    }

    let pp = unsafe {
        let buff = std::fs::read("tests/pub_params_dev.bin")
            .expect("Error reading from PubParams file");
        PublicParameters::from_slice_unchecked(buff.as_slice())
            .expect("PubParams deser error")
    };

    rusk_profile::set_common_reference_string("tests/pub_params_dev.bin")
        .expect("Error setting CRS in rusk_profile");
    let mut circuit = ExecuteCircuit::<17, 15>::create_dummy_circuit::<_, MS>(
        &mut rand::thread_rng(),
        1,
        1,
    )
    .expect("Error creating a dummy setup");

    let (pk, vk) = circuit.compile(&pp).expect("Error compiling the circuit");
    let proof = circuit
        .gen_proof(&pp, &pk, b"dusk")
        .expect("Error generating the proof");

    let mut pi = circuit.get_pi_positions().clone();
    // Reset PI positions to emulate real-world verification
    pi.iter_mut().for_each(|p| match p {
        PublicInput::BlsScalar(_, p) => *p = 0,
        PublicInput::JubJubScalar(_, p) => *p = 0,
        PublicInput::AffinePoint(_, p, q) => {
            *p = 0;
            *q = 0;
        }
    });

    let public_inputs_bytes: Vec<u8> =
        pi.iter().flat_map(|&inp| inp.to_bytes().to_vec()).collect();
    let label = String::from("transfer-execute-1-1").into_bytes();
    let proof_bytes = proof.to_bytes().to_vec();
    let vk_bytes = vk.to_bytes().to_vec();

    assert!({
        let mut circuit =
            ExecuteCircuit::<17, 15>::create_dummy_circuit::<_, MS>(
                &mut rand::thread_rng(),
                1,
                1,
            )
            .expect("Error creating a dummy setup");
        circuit.verify_proof(&pp, &vk, b"dusk", &proof, &pi).is_ok()
    });

    assert_eq!(
        true,
        network
            .query::<_, bool>(
                contract_id,
                (
                    verify_proof::PROOF_VERIFICATION,
                    proof_bytes,
                    vk_bytes,
                    label,
                    public_inputs_bytes
                ),
                &mut gas
            )
            .unwrap()
    );

    // If we stored the old CRS, let's restore it at the end of the test, too.
    if std::fs::metadata(old_crs_file_name).is_ok() {
        rusk_profile::set_common_reference_string(old_crs_file_name)
            .expect("Error restoring CRS in rusk_profile");
        std::fs::remove_file(old_crs_file_name)
            .expect("Could not remove temporary CRS holder file");
    }
}
