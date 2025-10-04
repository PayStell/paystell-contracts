use soroban_sdk::contracterror;
use core::fmt;

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum PaymentError {
    NotAuthorized = 1,
    MerchantNotFound = 2,
    InvalidSignature = 3,
    NonceAlreadyUsed = 4,
    InvalidAmount = 5,
    OrderExpired = 6,
    InvalidToken = 7,
    InvalidName = 8,
    InvalidDescription = 9,
    InvalidContactInfo = 10,
    MerchantAlreadyExists = 11,
    TransactionLimitExceeded = 12,
    InvalidTransactionLimit = 13,
    MerchantInactive = 14,
}

impl fmt::Display for PaymentError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PaymentError::NotAuthorized => write!(f, "Not authorized"),
            PaymentError::MerchantNotFound => write!(f, "Merchant not found or inactive"),
            PaymentError::InvalidSignature => write!(f, "Invalid merchant signature"),
            PaymentError::NonceAlreadyUsed => write!(f, "Payment nonce already used"),
            PaymentError::InvalidAmount => write!(f, "Invalid amount"),
            PaymentError::OrderExpired => write!(f, "Payment order has expired"),
            PaymentError::InvalidToken => write!(f, "Token not supported by merchant"),
            PaymentError::InvalidName => write!(f, "Invalid merchant name (must be 1-100 characters)"),
            PaymentError::InvalidDescription => write!(f, "Invalid description (max 500 characters)"),
            PaymentError::InvalidContactInfo => write!(f, "Invalid contact info (max 200 characters)"),
            PaymentError::MerchantAlreadyExists => write!(f, "Merchant already registered"),
            PaymentError::TransactionLimitExceeded => write!(f, "Transaction amount exceeds merchant limit"),
            PaymentError::InvalidTransactionLimit => write!(f, "Invalid transaction limit (must be positive)"),
            PaymentError::MerchantInactive => write!(f, "Merchant account is inactive"),
        }
    }
} 