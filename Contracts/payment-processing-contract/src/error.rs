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
    // Multi-signature specific errors
    PaymentNotFound = 8,
    InvalidThreshold = 9,
    ThresholdNotMet = 10,
    AlreadyExecuted = 11,
    AlreadyCancelled = 12,
    PaymentExpired = 13,
    AlreadySigned = 14,
    NotASigner = 15,
    InvalidStatus = 16,
    EmptySignersList = 17,
    DuplicateSigner = 18,
    InvalidPaymentId = 19,
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
            PaymentError::PaymentNotFound => write!(f, "Multi-signature payment not found"),
            PaymentError::InvalidThreshold => write!(f, "Invalid threshold value"),
            PaymentError::ThresholdNotMet => write!(f, "Signature threshold not met"),
            PaymentError::AlreadyExecuted => write!(f, "Payment already executed"),
            PaymentError::AlreadyCancelled => write!(f, "Payment already cancelled"),
            PaymentError::PaymentExpired => write!(f, "Payment has expired"),
            PaymentError::AlreadySigned => write!(f, "Already signed by this signer"),
            PaymentError::NotASigner => write!(f, "Not a valid signer for this payment"),
            PaymentError::InvalidStatus => write!(f, "Invalid payment status"),
            PaymentError::EmptySignersList => write!(f, "Signers list cannot be empty"),
            PaymentError::DuplicateSigner => write!(f, "Duplicate signer in list"),
            PaymentError::InvalidPaymentId => write!(f, "Invalid payment ID"),
        }
    }
}