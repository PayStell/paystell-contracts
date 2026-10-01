use soroban_sdk::{contract, address, vec, Vec, Env, Symbol};

#[contract]
pub struct MultisigManager;

#[derive(Clone)]
pub enum MultisigThreshold {
    Absolute(u32), // Fixed number of signatures
    Percentage(u32), // Percentage of total signers (1-100)
}

#[derive(Clone)]
pub struct MultisigConfig {
    pub threshold: MultisigThreshold,
    pub signers: Vec<address>,
    pub version: u32,
}

#[derive(Clone)]
pub struct PendingPayment {
    pub payment_id: Symbol,
    pub amount: i128,
    pub destination: address,
    pub signatures: Vec<address>,
    pub executed: bool,
}

#[derive(Clone)]
pub struct SignerInfo {
    pub weight: u32,
    pub active: bool,
}

impl MultisigManager {
    pub fn initialize(env: Env, threshold: MultisigThreshold, initial_signers: Vec<address>) {
        if initial_signers.is_empty() {
            panic!("At least one signer is required");
        }
        
        let config = MultisigConfig {
            threshold,
            signers: initial_signers,
            version: 1,
        };
        
        env.storage().instance().set(&Symbol::new("config"), &config);
    }

    pub fn add_signer(env: Env, caller: address, new_signer: address) {
        Self::verify_admin(env, caller);
        
        let mut config: MultisigConfig = env.storage().instance().get(&Symbol::new("config")).unwrap();
        if config.signers.contains(&new_signer) {
            panic!("Signer already exists");
        }
        
        config.signers.push(new_signer);
        config.version += 1;
        env.storage().instance().set(&Symbol::new("config"), &config);
    }

    pub fn remove_signer(env: Env, caller: address, signer_to_remove: address) {
        Self::verify_admin(env, caller);
        
        let mut config: MultisigConfig = env.storage().instance().get(&Symbol::new("config")).unwrap();
        let index = config.signers.iter().position(|&s| s == signer_to_remove)
            .expect("Signer not found");
        
        config.signers.remove(index);
        config.version += 1;
        env.storage().instance().set(&Symbol::new("config"), &config);
    }

    pub fn update_threshold(env: Env, caller: address, new_threshold: MultisigThreshold) {
        Self::verify_admin(env, caller);
        
        let mut config: MultisigConfig = env.storage().instance().get(&Symbol::new("config")).unwrap();
        config.threshold = new_threshold;
        config.version += 1;
        env.storage().instance().set(&Symbol::new("config"), &config);
    }

    pub fn create_pending_payment(env: Env, caller: address, payment_id: Symbol, amount: i128, destination: address) {
        let config: MultisigConfig = env.storage().instance().get(&Symbol::new("config")).unwrap();
        if !config.signers.contains(&caller) {
            panic!("Only authorized signers can initiate payments");
        }

        let payment = PendingPayment {
            payment_id: payment_id.clone(),
            amount,
            destination,
            signatures: vec![&caller],
            executed: false,
        };

        env.storage().persistent().set(&payment_id, &payment);
    }

    pub fn sign_payment(env: Env, caller: address, payment_id: Symbol) {
        let config: MultisigConfig = env.storage().instance().get(&Symbol::new("config")).unwrap();
        if !config.signers.contains(&caller) {
            panic!("Unauthorized signer");
        }

        let mut payment: PendingPayment = env.storage().persistent().get(&payment_id).expect("Payment not found");
        if payment.executed {
            panic!("Payment already executed");
        }
        if payment.signatures.contains(&caller) {
            panic!("Signer already signed this payment");
        }

        payment.signatures.push(caller);
        env.storage().persistent().set(&payment_id, &payment);
    }

    pub fn verify_threshold_met(env: Env, payment_id: Symbol) -> bool {
        let config: MultisigConfig = env.storage().instance().get(&Symbol::new("config")).unwrap();
        let payment: PendingPayment = env.storage().persistent().get(&payment_id).expect("Payment not found");
        
        let sig_count = payment.signatures.len() as u32;
        
        match config.threshold {
            MultisigThreshold::Absolute(min) => sig_count >= min,
            MultisigThreshold::Percentage(perc) => {
                let total = config.signers.len() as u32;
                let required = (total * perc + 99) / 100; // Ceiling division
                sig_count >= required
            }
        }
    }

    fn verify_admin(env: Env, caller: address) {
        let config: MultisigConfig = env.storage().instance().get(&Symbol::new("config")).unwrap();
        // For this implementation, any authorized signer can propose changes, 
        // but in a production environment, this would require a separate admin role 
        // or a multi-sig vote to change the config itself.
        if !config.signers.contains(&caller) {
            panic!("Unauthorized: Caller is not a registered signer");
        }
    }
}
