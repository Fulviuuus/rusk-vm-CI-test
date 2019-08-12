use std::collections::HashMap;

use crate::digest::Digest;
use crate::transaction::Transaction;
use crate::wallet::ManagedAccount;

//use wasmi::{ExternVal, ImportsBuilder, ModuleInstance, RuntimeValue};
//use host_fns::{HostExternals, HostImportResolver};

struct Block {
    height: u128,
    transactions: Vec<Transaction>,
}

#[derive(Default)]
pub struct Account {
    nonce: u128,
    balance: u128,
}

pub struct Contract {
    #[allow(unused)]
    balance: u128,
    code: Vec<u8>,
}

impl Account {
    pub fn balance(&self) -> u128 {
        self.balance
    }

    pub fn nonce(&self) -> u128 {
        self.nonce
    }

    pub fn balance_mut(&mut self) -> &mut u128 {
        &mut self.balance
    }

    pub fn nonce_mut(&mut self) -> &mut u128 {
        &mut self.nonce
    }
}

#[derive(Default)]
pub struct NetworkState {
    accounts: HashMap<Digest, Account>,
    contracts: HashMap<Digest, Contract>,
    blocks: Vec<Block>,
    queue: Vec<Transaction>,
}

impl NetworkState {
    pub fn genesis(mint: &ManagedAccount) -> Self {
        let account_id = mint.id();
        let mut accounts = HashMap::new();
        accounts.insert(
            account_id.clone(),
            Account {
                balance: 1_000_000,
                nonce: 0,
            },
        );
        NetworkState {
            accounts,
            contracts: HashMap::new(),
            blocks: vec![],
            queue: vec![],
        }
    }

    pub fn get_account(&self, account_id: &Digest) -> Option<&Account> {
        self.accounts.get(account_id)
    }

    // Gets a mutable reference to the account id, creating it if it does not already exist
    pub fn get_account_mut(&mut self, account_id: &Digest) -> &mut Account {
        self.accounts.entry(*account_id).or_default()
    }

    pub fn mint_block(&mut self) {
        let mut block = vec![];
        let mut queue = std::mem::replace(&mut self.queue, vec![]);
        for tcn in queue.drain(..) {
            if tcn.valid(self) {
                tcn.apply(self);
                block.push(tcn);
            }
        }
        self.append_transaction_block(block);
    }

    fn append_transaction_block(&mut self, _transactions: Vec<Transaction>) {
        ()
    }

    pub fn queue_transaction(&mut self, transaction: Transaction) {
        self.queue.push(transaction)
    }

    // pub fn new_contract(
    //     &mut self,
    //     bytecode: &[u8],
    // ) -> Result<Digest, wasmi::Error> {
    //     let module = wasmi::Module::from_buffer(bytecode)?;
    //     module.deny_floating_point()?;

    //     let hash = Digest::new(bytecode);
    //     self.contracts.insert(hash.clone(), module);

    //     Ok(hash)
    // }

    // pub fn call(
    //     &mut self,
    //     contract_hash: &Digest,
    //     method: &str,
    // ) -> Result<Option<RuntimeValue>, wasmi::Error> {
    //     if let Some(contract) = self.contracts.get(&contract_hash) {
    //         let imports =
    //             ImportsBuilder::new().with_resolver("env", &HostImportResolver);

    //         let instance =
    //             ModuleInstance::new(&contract, &imports)?.assert_no_start();

    //         // Get memory reference for call
    //         match instance.export_by_name("memory") {
    //             Some(ExternVal::Memory(memref)) => {
    //                 let mut externals = HostExternals(memref.clone());
    //                 instance.invoke_export(method, &[], &mut externals)
    //             }
    //             _ => panic!("No memory available"),
    //         }
    //     } else {
    //         panic!("no contract")
    //     }
    // }
}
