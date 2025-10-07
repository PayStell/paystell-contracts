use core::fmt;
use soroban_sdk::contracterror;

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum PaymentError {
    // Common
    NotAuthorized = 1,
    MerchantNotFound = 2,
    InvalidSignature = 3,
    NonceAlreadyUsed = 4,
    InvalidAmount = 5,
    OrderExpired = 6,
    InvalidToken = 7,

    // Admin / contract management
    AdminNotFound = 8,
    ContractPaused = 9,
    AlreadyPaused = 10,
    InvalidFeeRate = 11,

    // Multi-signature specific errors
    PaymentNotFound = 12,
    InvalidThreshold = 13,
    ThresholdNotMet = 14,
    AlreadyExecuted = 15,
    AlreadyCancelled = 16,
    PaymentExpired = 17,
    AlreadySigned = 18,
    NotASigner = 19,
    InvalidStatus = 20,
    EmptySignersList = 21,
    DuplicateSigner = 22,
    InvalidPaymentId = 23,
}

impl fmt::Display for PaymentError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            // Common
            PaymentError::NotAuthorized => write!(f, "Not authorized"),
            PaymentError::MerchantNotFound => write!(f, "Merchant not found or inactive"),
            PaymentError::InvalidSignature => write!(f, "Invalid merchant signature"),
            PaymentError::NonceAlreadyUsed => write!(f, "Payment nonce already used"),
            PaymentError::InvalidAmount => write!(f, "Invalid amount"),
            PaymentError::OrderExpired => write!(f, "Payment order has expired"),
            PaymentError::InvalidToken => write!(f, "Token not supported by merchant"),

            // Admin / contract management
            PaymentError::AdminNotFound => write!(f, "Admin not found"),
            PaymentError::ContractPaused => write!(f, "Contract is paused"),
            PaymentError::AlreadyPaused => write!(f, "Contract is already paused"),
            PaymentError::InvalidFeeRate => write!(f, "Invalid fee rate"),

            // Multi-signature
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

