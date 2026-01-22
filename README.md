# PayStell: scalable, secure, and decentralized smart contracts for Soroban Stellar.

This README file provides documentation for the smart contracts.

## Overview

PayStell is a comprehensive payment processing smart contract for the Stellar Soroban network, featuring merchant management, payment processing, refunds, multi-signature payments, and advanced payment history query capabilities.

## Prerequisites

Before you begin, ensure you have the following installed:

1. **Rust** (latest stable version): https://www.rust-lang.org/tools/install
2. **Stellar CLI** (Soroban CLI): https://soroban.stellar.org/docs/getting-started/setup#install-the-soroban-cli
3. **Docker Desktop** (for local network): https://www.docker.com/products/docker-desktop

Verify installations:

```bash
rustc --version
cargo --version
soroban --version
docker --version
```

## Setup

### 1. Clone the Repository

```bash
git clone <repository-url>
cd paystell-contracts
```

### 2. Build the Contract

Navigate to the payment processing contract directory:

```bash
cd Contracts/payment-processing-contract
```

Build the contract:

```bash
cargo build --target wasm32-unknown-unknown --release
```

The compiled WASM file will be located at:
```
target/wasm32-unknown-unknown/release/payment_processing_contract.wasm
```

## Testing

### Run All Tests

From the contract directory:

```bash
cargo test
```

### Run Specific Test Suites

```bash
# Test payment processing
cargo test test_successful_payment_with_signature

# Test refund functionality
cargo test test_successful_refund_flow

# Test payment history queries
cargo test test_get_merchant_payment_history

# Test multi-signature payments
cargo test test_initiate_multisig_payment_success
```

### Test Coverage

The contract includes comprehensive tests for:
- Merchant registration and management
- Payment processing with signatures
- Refund workflows
- Multi-signature payments
- Payment history queries
- Authorization and access control
- Payment archiving and cleanup

## Local Network Setup

### 1. Start Local Stellar Network

```bash
# Start Docker Desktop first, then:
soroban network container start local
```

### 2. Deploy the Contract

```bash
# From the contract directory
soroban contract deploy \
  --wasm target/wasm32-unknown-unknown/release/payment_processing_contract.wasm \
  --source-account <your-account-secret-key> \
  --network local
```

Save the contract ID that is returned.

### 3. Initialize the Contract

```bash
# Set admin (replace CONTRACT_ID with your deployed contract ID)
soroban contract invoke \
  --id CONTRACT_ID \
  --source-account <admin-secret-key> \
  --network local \
  -- set_admin \
  --admin <admin-address>
```

## Usage Examples

### Payment Processing Contract

The `payment-processing-contract` supports:
- Merchant registration and management
- Payment processing with signature verification
- Refund management
- Multi-signature payments
- Payment history queries with pagination and filtering

#### Core Functions

**Merchant Management:**
```bash
# Register a merchant
soroban contract invoke \
  --id CONTRACT_ID \
  --source-account <merchant-secret-key> \
  --network local \
  -- register_merchant \
  --merchant_address <merchant-address> \
  --name "My Store" \
  --description "Store description" \
  --contact_info "contact@store.com" \
  --category Retail
```

**Payment Processing:**
```bash
# Process a payment (requires signature)
soroban contract invoke \
  --id CONTRACT_ID \
  --source-account <payer-secret-key> \
  --network local \
  -- process_payment_with_signature \
  --payer <payer-address> \
  --order '{"merchant_address":"...","amount":1000,...}' \
  --signature <signature-bytes> \
  --merchant_public_key <public-key>
```

#### Payment History Queries

**Get Merchant Payment History:**
```bash
soroban contract invoke \
  --id CONTRACT_ID \
  --source-account <merchant-secret-key> \
  --network local \
  -- get_merchant_payment_history \
  --merchant <merchant-address> \
  --cursor null \
  --limit 10 \
  --filter null \
  --sort_field Date \
  --sort_order Descending
```

**Get Payer Payment History:**
```bash
soroban contract invoke \
  --id CONTRACT_ID \
  --source-account <payer-secret-key> \
  --network local \
  -- get_payer_payment_history \
  --payer <payer-address> \
  --cursor null \
  --limit 10 \
  --filter '{"amount_min":100,"amount_max":1000,"status":"Any"}' \
  --sort_field Amount \
  --sort_order Ascending
```

**Get Payment by ID:**
```bash
soroban contract invoke \
  --id CONTRACT_ID \
  --source-account <caller-secret-key> \
  --network local \
  -- get_payment_by_id \
  --caller <caller-address> \
  --order_id "ORDER_123"
```

**Get Global Payment Statistics (Admin only):**
```bash
soroban contract invoke \
  --id CONTRACT_ID \
  --source-account <admin-secret-key> \
  --network local \
  -- get_global_payment_stats \
  --admin <admin-address> \
  --date_start null \
  --date_end null
```

#### Payment Management

**Update Payment Status:**
```bash
soroban contract invoke \
  --id CONTRACT_ID \
  --source-account <merchant-secret-key> \
  --network local \
  -- update_payment_status \
  --caller <merchant-address> \
  --order_id "ORDER_123" \
  --refunded_amount 500
```

**Archive Payment Record (Admin only):**
```bash
soroban contract invoke \
  --id CONTRACT_ID \
  --source-account <admin-secret-key> \
  --network local \
  -- archive_payment_record \
  --admin <admin-address> \
  --order_id "ORDER_123"
```

**Cleanup Expired Payments (Admin only):**
```bash
soroban contract invoke \
  --id CONTRACT_ID \
  --source-account <admin-secret-key> \
  --network local \
  -- cleanup_expired_payments \
  --admin <admin-address>
```

**Set Cleanup Period (Admin only):**
```bash
# Set cleanup period to 90 days (in seconds)
soroban contract invoke \
  --id CONTRACT_ID \
  --source-account <admin-secret-key> \
  --network local \
  -- set_payment_cleanup_period \
  --admin <admin-address> \
  --period 7776000
```

#### Refunds

The contract implements a refund system with the following:

- Initiate refund: merchants or payers may open a refund request for a paid order
- Approve/Reject: merchant or admin may approve/reject
- Execute: transfers funds from merchant to payer atomically
- Status query: get current status (Pending, Approved, Rejected, Completed)

**Refund APIs:**

```bash
# Initiate refund
soroban contract invoke \
  --id CONTRACT_ID \
  --source-account <caller-secret-key> \
  --network local \
  -- initiate_refund \
  --caller <caller-address> \
  --refund_id "REFUND_001" \
  --order_id "ORDER_123" \
  --amount 500 \
  --reason "Customer request"

# Approve refund
soroban contract invoke \
  --id CONTRACT_ID \
  --source-account <merchant-secret-key> \
  --network local \
  -- approve_refund \
  --caller <merchant-address> \
  --refund_id "REFUND_001"

# Execute refund
soroban contract invoke \
  --id CONTRACT_ID \
  --source-account <merchant-secret-key> \
  --network local \
  -- execute_refund \
  --refund_id "REFUND_001"
```

**Refund Rules:**
- Refund window: 30 days from `paid_at`
- Amount: partial refunds allowed; cumulative refunds cannot exceed original
- Authorization: merchant or payer can initiate; merchant or admin can approve/reject
- Insufficient balance on merchant results in failure
- Events: `refund_initiated`, `refund_approved`, `refund_rejected`, `refund_executed`

## Payment History Query Features

### Filtering Options

- **Date Range**: Filter payments by `paid_at` timestamp
- **Amount Range**: Filter by payment amount (min/max)
- **Token**: Filter by specific token address
- **Status**: Filter by payment status (Any, Completed, PartiallyRefunded, FullyRefunded)
  - Use `Any` to include all statuses (no filter)

### Sorting Options

- **Field**: Sort by `Date` or `Amount`
- **Order**: `Ascending` or `Descending`

### Pagination

- **Cursor-based**: Use `order_id` as cursor for efficient pagination
- **Limit**: Maximum 100 results per query (configurable)
- **Next Cursor**: Returned in query results for subsequent pages

### Authorization

- **Merchants**: Can only query their own payment history
- **Payers**: Can only query their own payment history
- **Admins**: Can query all payments and access global statistics

## Development Workflow

### 1. Make Changes

Edit the contract source files in `src/`:
- `lib.rs` - Main contract implementation
- `types.rs` - Data structures
- `storage.rs` - Storage operations
- `error.rs` - Error definitions
- `helper.rs` - Validation helpers

### 2. Test Locally

```bash
cargo test
```

### 3. Build for Deployment

```bash
cargo build --target wasm32-unknown-unknown --release
```

### 4. Deploy to Testnet

```bash
# Deploy to Stellar Testnet
soroban contract deploy \
  --wasm target/wasm32-unknown-unknown/release/payment_processing_contract.wasm \
  --source-account <testnet-account-secret-key> \
  --network testnet
```

## Project Structure

```
paystell-contracts/
├── Contracts/
│   └── payment-processing-contract/
│       ├── Cargo.toml
│       └── src/
│           ├── lib.rs          # Main contract implementation
│           ├── types.rs        # Data structures and types
│           ├── storage.rs      # Storage operations
│           ├── error.rs        # Error definitions
│           ├── helper.rs       # Validation helpers
│           └── test.rs         # Unit tests
├── Cargo.toml
└── README.md
```

## Troubleshooting

### Common Issues

1. **Build Errors**: Ensure you're using the correct Rust version and target:
   ```bash
   rustup target add wasm32-unknown-unknown
   ```

2. **Network Connection**: If local network fails, restart Docker and try:
   ```bash
   soroban network container restart local
   ```

3. **Contract Deployment**: Ensure you have sufficient XLM in your account for fees

4. **Test Failures**: Check that all dependencies are correctly specified in `Cargo.toml`

## Additional Resources

- [Soroban Documentation](https://soroban.stellar.org/docs)
- [Stellar Documentation](https://developers.stellar.org/docs)
- [Rust Documentation](https://doc.rust-lang.org/)

## License

This project is licensed under the MIT License. See the LICENSE file for more details.

