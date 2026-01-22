use soroban_sdk::String;
use crate::error::PaymentError;
use crate::types::PaymentQueryFilter;

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

// Query validation constants
pub const MAX_QUERY_LIMIT: u32 = 100;
pub const MAX_DATE_RANGE_DAYS: u64 = 365; // Maximum 1 year range

/// Validates query limit (max 100 results)
pub fn validate_query_limit(limit: u32) -> Result<(), PaymentError> {
    if limit == 0 || limit > MAX_QUERY_LIMIT {
        return Err(PaymentError::InvalidQueryLimit);
    }
    Ok(())
}

/// Validates date range parameters
pub fn validate_date_range(date_start: Option<u64>, date_end: Option<u64>) -> Result<(), PaymentError> {
    if let (Some(start), Some(end)) = (date_start, date_end) {
        if end < start {
            return Err(PaymentError::InvalidDateRange);
        }
        // Check reasonable range (max 1 year)
        let range_seconds = end - start;
        let max_range_seconds = MAX_DATE_RANGE_DAYS * 24 * 60 * 60;
        if range_seconds > max_range_seconds {
            return Err(PaymentError::InvalidDateRange);
        }
    }
    Ok(())
}

/// Validates amount range parameters
pub fn validate_amount_range(amount_min: Option<i128>, amount_max: Option<i128>) -> Result<(), PaymentError> {
    if let (Some(min), Some(max)) = (amount_min, amount_max) {
        if min < 0 || max < 0 {
            return Err(PaymentError::InvalidAmount);
        }
        if max < min {
            return Err(PaymentError::InvalidAmount);
        }
    } else if let Some(min) = amount_min {
        if min < 0 {
            return Err(PaymentError::InvalidAmount);
        }
    } else if let Some(max) = amount_max {
        if max < 0 {
            return Err(PaymentError::InvalidAmount);
        }
    }
    Ok(())
}

/// Validates cursor (order_id) exists
pub fn validate_cursor(_env: &soroban_sdk::Env, cursor: &Option<String>, storage: &crate::storage::Storage) -> Result<(), PaymentError> {
    if let Some(ref order_id) = cursor {
        // Check if payment exists
        storage.get_payment(order_id).map(|_| ()).map_err(|_| PaymentError::InvalidCursor)?;
    }
    Ok(())
}

/// Validates query filter parameters
pub fn validate_query_filter(filter: &PaymentQueryFilter) -> Result<(), PaymentError> {
    validate_date_range(filter.date_start, filter.date_end)?;
    validate_amount_range(filter.amount_min, filter.amount_max)?;
    Ok(())
}
