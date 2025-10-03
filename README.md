# PayStell: scalable, secure, and decentralized smart contracts for Soroban Stellar.

This README file provides documentation for the smart contracts.

## Overview



## Usage

To use this smart contract, follow these steps:

1. Ensure you have the necessary dependencies installed.
2. Compile the contract using `cargo build`.
3. Deploy the contract to your desired blockchain network.

### Payment Processing Contract

The `payment-processing-contract` supports registering merchants, adding supported tokens, processing signed payments, and refunds.

#### Refunds

The contract implements a refund system with the following:

- Initiate refund: merchants or payers may open a refund request for a paid order
- Approve/Reject: merchant or admin may approve/reject
- Execute: transfers funds from merchant to payer atomically
- Status query: get current status (Pending, Approved, Rejected, Completed)

APIs:

- `set_admin(admin: Address)`
- `initiate_refund(caller: Address, refund_id: String, order_id: String, amount: i128, reason: String)`
- `approve_refund(caller: Address, refund_id: String)`
- `reject_refund(caller: Address, refund_id: String)`
- `execute_refund(refund_id: String)`
- `get_refund_status(refund_id: String) -> RefundStatus`

Rules and validations:

- Refund window: 30 days from `paid_at`
- Amount: partial refunds allowed; cumulative refunds cannot exceed original
- Authorization: merchant or payer can initiate; merchant or admin can approve/reject
- Insufficient balance on merchant results in failure
- Events: `refund_initiated`, `refund_approved`, `refund_rejected`, `refund_executed`

## Testing

Integration tests for this contract can be found in the `tests` directory. Run the tests using:

```
cargo test
```

## License

This project is licensed under the MIT. See the LICENSE file for more details.

