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

    // Merchant profile validation errors
    InvalidName = 8,
    InvalidDescription = 9,
    InvalidContactInfo = 10,
    MerchantAlreadyExists = 11,
    TransactionLimitExceeded = 12,
    InvalidTransactionLimit = 13,
    MerchantInactive = 14,

    // Admin / contract management
    AdminNotFound = 15,
    ContractPaused = 16,
    AlreadyPaused = 17,
    InvalidFeeRate = 18,

    // Multi-signature specific errors
    PaymentNotFound = 19,
    InvalidThreshold = 20,
    ThresholdNotMet = 21,
    AlreadyExecuted = 22,
    AlreadyCancelled = 23,
    PaymentExpired = 24,
    AlreadySigned = 25,
    NotASigner = 26,
    InvalidStatus = 27,
    EmptySignersList = 28,
    DuplicateSigner = 29,
    InvalidPaymentId = 30,

    // Refund specific errors
    RefundNotFound = 31,
    NotRefundable = 32,
    RefundWindowExceeded = 33,
    ExceedsOriginalAmount = 34,
    InvalidRefundStatus = 35,
    InsufficientBalance = 36,
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

            // Merchant profile validation errors
            PaymentError::InvalidName => write!(f, "Invalid merchant name (must be 1-100 characters)"),
            PaymentError::InvalidDescription => write!(f, "Invalid description (max 500 characters)"),
            PaymentError::InvalidContactInfo => write!(f, "Invalid contact info (max 200 characters)"),
            PaymentError::MerchantAlreadyExists => write!(f, "Merchant already registered"),
            PaymentError::TransactionLimitExceeded => write!(f, "Transaction amount exceeds merchant limit"),
            PaymentError::InvalidTransactionLimit => write!(f, "Invalid transaction limit (must be positive)"),
            PaymentError::MerchantInactive => write!(f, "Merchant account is inactive"),

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

            // Refund errors
            PaymentError::RefundNotFound => write!(f, "Refund not found"),
            PaymentError::NotRefundable => write!(f, "Payment not refundable"),
            PaymentError::RefundWindowExceeded => write!(f, "Refund window exceeded"),
            PaymentError::ExceedsOriginalAmount => write!(f, "Refund exceeds original amount"),
            PaymentError::InvalidRefundStatus => write!(f, "Invalid refund status transition"),
            PaymentError::InsufficientBalance => write!(f, "Insufficient balance for refund"),
        }
    }
}

