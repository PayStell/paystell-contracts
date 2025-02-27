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
        }
    }
} 