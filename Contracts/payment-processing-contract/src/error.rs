use soroban_sdk::contracterror;
use core::fmt;

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum PaymentError {
    NotAuthorized = 1,
    MerchantNotFound = 2,
    InvalidPaymentLink = 3,
    PaymentAlreadyProcessed = 4,
    InvalidAmount = 5,
}

impl fmt::Display for PaymentError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PaymentError::PaymentAlreadyProcessed => write!(f, "Payment has already been processed"),
            PaymentError::NotAuthorized => write!(f, "Not authorized"),
            PaymentError::MerchantNotFound => write!(f, "Merchant not found"),
            PaymentError::InvalidPaymentLink => write!(f, "Invalid payment link"),
            PaymentError::InvalidAmount => write!(f, "Invalid amount"),
        }
    }
} 