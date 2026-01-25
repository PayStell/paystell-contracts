use crate::error::PaymentError;
use soroban_sdk::String;

// Validation constants
pub const MIN_NAME_LENGTH: u32 = 1;
pub const MAX_NAME_LENGTH: u32 = 100;
pub const MAX_DESCRIPTION_LENGTH: u32 = 500;
pub const MAX_CONTACT_INFO_LENGTH: u32 = 200;
pub const DEFAULT_TRANSACTION_LIMIT: i128 = 1_000_000_000_000; // 1 trillion stroops (100,000 XLM)

/// Validates merchant name length
pub fn validate_name(name: &String) -> Result<(), PaymentError> {
    let len = name.len();
    if len < MIN_NAME_LENGTH || len > MAX_NAME_LENGTH {
        return Err(PaymentError::InvalidName);
    }
    Ok(())
}

/// Validates merchant description length
pub fn validate_description(description: &String) -> Result<(), PaymentError> {
    if description.len() > MAX_DESCRIPTION_LENGTH {
        return Err(PaymentError::InvalidDescription);
    }
    Ok(())
}

/// Validates merchant contact information length
pub fn validate_contact_info(contact_info: &String) -> Result<(), PaymentError> {
    if contact_info.len() > MAX_CONTACT_INFO_LENGTH {
        return Err(PaymentError::InvalidContactInfo);
    }
    Ok(())
}

/// Validates transaction limit is positive
pub fn validate_transaction_limit(limit: i128) -> Result<(), PaymentError> {
    if limit <= 0 {
        return Err(PaymentError::InvalidTransactionLimit);
    }
    Ok(())
}
