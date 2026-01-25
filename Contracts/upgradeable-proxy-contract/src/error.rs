use soroban_sdk::contracterror;

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum ProxyError {
    AlreadyInitialized = 1,
    InvalidAdmins = 2,
    InvalidThreshold = 3,
    NotInitialized = 4,
    NotAdmin = 5,
    ThresholdNotMet = 6,
    DelayNotPassed = 7,
    AlreadyExecuted = 8,
    ProposalNotFound = 9,
    NoRollbackAvailable = 10,
    StorageError = 11,
    ImplementationNotSet = 12,
    ValidationFailed = 13,
    MigrationFailed = 14,
    SameImplementation = 15,
    MetadataTooLarge = 16,
    InvalidImplementation = 17,
    RollbackFailed = 18,}