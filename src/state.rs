// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
//
// Copyright (c) DUSK NETWORK. All rights reserved.

use std::cell::RefCell;
use std::collections::HashMap;
use std::ops::{Deref, DerefMut};
use std::rc::Rc;

use canonical::{Canon, Ident, Sink, Source, Store};
use dusk_abi::{Query, Transaction};
use dusk_kelvin_map::Map;

use crate::call_context::CallContext;
use crate::contract::{Contract, ContractId};
use crate::gas::GasMeter;
use crate::VMError;

/// The trait that host function modules use to communicate with the VM
pub trait HostModule {
    // ??
}

/// The main network state, includes the full state of contracts.
#[derive(Clone, Default)]
pub struct NetworkState<S>
where
    S: Store,
{
    block_height: u64,
    contracts: Map<ContractId, Contract, S>,
    modules: Rc<RefCell<HashMap<ContractId, Box<dyn HostModule>>>>,
    store: S,
}

// Manual implementation of `Canon` to ignore the "modules" which needs to be
// re-instantiated on program initialization.
impl<S> Canon<S> for NetworkState<S>
where
    S: Store,
{
    fn write(&self, sink: &mut impl Sink<S>) -> Result<(), S::Error> {
        self.block_height.write(sink)?;
        self.contracts.write(sink)
    }

    fn read(source: &mut impl Source<S>) -> Result<Self, S::Error> {
        let block_height = u64::read(source)?;
        let contracts = Map::read(source)?;
        Ok(NetworkState {
            block_height,
            contracts,
            store: source.store().clone(),
            modules: Rc::new(RefCell::new(HashMap::new())),
        })
    }

    fn encoded_len(&self) -> usize {
        Canon::<S>::encoded_len(&self.block_height)
            + Canon::<S>::encoded_len(&self.contracts)
    }
}

impl<S> NetworkState<S>
where
    S: Store,
{
    /// Returns a [`NetworkState`] for a specific block height
    pub fn with_block_height(block_height: u64) -> Self {
        Self {
            block_height,
            contracts: Map::default(),
            modules: Rc::new(RefCell::new(HashMap::new())),
            store: S::default(),
        }
    }

    /// Deploys a contract to the state, returns the address of the created
    /// contract or an error
    pub fn deploy(
        &mut self,
        contract: Contract,
    ) -> Result<ContractId, S::Error> {
        let id: ContractId = S::Ident::from_bytes(contract.bytecode()).into();

        self.contracts
            .insert(id.clone(), contract)
            .expect("FIXME: error handling");
        Ok(id)
    }

    /// Returns a reference to the specified contracts state
    pub fn get_contract<'a>(
        &'a self,
        contract_id: &ContractId,
    ) -> Result<Option<impl Deref<Target = Contract> + 'a>, VMError<S>> {
        self.contracts
            .get(contract_id)
            .map_err(VMError::from_store_error)
    }

    /// Returns a reference to the specified contracts state
    pub fn get_contract_mut<'a>(
        &'a mut self,
        contract_id: &ContractId,
    ) -> Result<Option<impl DerefMut<Target = Contract> + 'a>, VMError<S>> {
        self.contracts
            .get_mut(contract_id)
            .map_err(VMError::from_store_error)
    }

    /// Returns a reference to the store backing the state
    pub fn store(&self) -> &S {
        &self.store
    }

    /// Returns the state's block height
    pub fn block_height(&self) -> u64 {
        self.block_height
    }

    /// Queryn the contract at address `target`
    pub fn query<A, R>(
        &mut self,
        target: ContractId,
        query: A,
        gas_meter: &mut GasMeter,
    ) -> Result<R, VMError<S>>
    where
        A: Canon<S>,
        R: Canon<S>,
    {
        let store = self.store().clone();
        let mut context = CallContext::new(self, gas_meter, &store)
            .expect("FIXME: error handling");

        let result = context.query(
            target,
            Query::from_canon(&query, &store)
                .map_err(VMError::from_store_error)?,
        )?;

        result.cast(store).map_err(VMError::from_store_error)
    }

    /// Transact with the contract at address `target`
    pub fn transact<A, R>(
        &mut self,
        target: ContractId,
        transaction: A,
        gas_meter: &mut GasMeter,
    ) -> Result<R, VMError<S>>
    where
        A: Canon<S>,
        R: Canon<S>,
    {
        let store = self.store().clone();
        let mut context = CallContext::new(self, gas_meter, &store)
            .expect("FIXME: error handling");

        let result = context.transact(
            target,
            Transaction::from_canon(&transaction, &store)
                .map_err(VMError::from_store_error)?,
        )?;

        result.cast(store).map_err(VMError::from_store_error)
    }

    /// Register a host-fn handler
    pub fn register_host_module<M>(&mut self, id: ContractId, module: M)
    where
        M: HostModule + 'static,
    {
        self.modules.borrow_mut().insert(id, Box::new(module));
    }
}
