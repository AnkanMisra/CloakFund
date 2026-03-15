// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

/// @title PrivacyPool - A Hash-Commit-Reveal Mixer (Tornado Cash Lite)
/// @notice Accepts standardized ETH deposits and releases them via cryptographic
///         commit-reveal proofs. Provides on-chain privacy by breaking the link
///         between the depositor address and the withdrawal recipient.
/// @dev Uses SHA-256 hashing instead of Groth16 ZK-SNARKs for hackathon MVP.
///      commitment = sha256(abi.encodePacked(secret, nullifier))
///
///      Flow:
///        1. Depositor calls deposit(commitment) with exactly DENOMINATION ETH
///        2. Withdrawer provides (secret, nullifier, recipient) to withdraw()
///        3. Contract verifies the commitment, checks nullifier uniqueness,
///           and transfers DENOMINATION ETH to the recipient
contract PrivacyPool {
    // ──────────────────────────────────────────────────────────────────────
    //  Constants
    // ──────────────────────────────────────────────────────────────────────

    /// @notice The fixed deposit denomination: 0.0001 ETH (100_000_000_000_000 wei)
    uint256 public constant DENOMINATION = 0.0001 ether;

    // ──────────────────────────────────────────────────────────────────────
    //  State
    // ──────────────────────────────────────────────────────────────────────

    /// @notice Mapping of valid commitments (sha256 hashes) deposited into the pool
    mapping(bytes32 => bool) public commitments;

    /// @notice Mapping of spent nullifiers to prevent double-withdrawal
    mapping(bytes32 => bool) public nullifiers;

    /// @notice Running count of deposits (used as leaf index in events)
    uint32 public nextLeafIndex;

    // ──────────────────────────────────────────────────────────────────────
    //  Events
    // ──────────────────────────────────────────────────────────────────────

    /// @notice Emitted when a deposit is made into the pool
    /// @param commitment  The SHA-256 commitment hash
    /// @param leafIndex   The sequential index of this deposit
    /// @param timestamp   Block timestamp of the deposit
    event Deposit(
        bytes32 indexed commitment,
        uint32 leafIndex,
        uint256 timestamp
    );

    /// @notice Emitted when a withdrawal is successfully processed
    /// @param recipient  The address that received the funds
    /// @param nullifier  The nullifier used (now marked as spent)
    event Withdrawal(
        address indexed recipient,
        bytes32 nullifier
    );

    // ──────────────────────────────────────────────────────────────────────
    //  Errors
    // ──────────────────────────────────────────────────────────────────────

    error InvalidDenomination(uint256 sent, uint256 required);
    error CommitmentAlreadyExists(bytes32 commitment);
    error CommitmentNotFound(bytes32 commitment);
    error NullifierAlreadySpent(bytes32 nullifier);
    error WithdrawTransferFailed();

    // ──────────────────────────────────────────────────────────────────────
    //  Deposit
    // ──────────────────────────────────────────────────────────────────────

    /// @notice Deposit ETH into the privacy pool by providing a commitment hash
    /// @param commitment  The hash: sha256(abi.encodePacked(secret, nullifier))
    /// @dev msg.value must equal DENOMINATION exactly
    function deposit(bytes32 commitment) external payable {
        if (msg.value != DENOMINATION) {
            revert InvalidDenomination(msg.value, DENOMINATION);
        }
        if (commitments[commitment]) {
            revert CommitmentAlreadyExists(commitment);
        }

        commitments[commitment] = true;

        uint32 leafIndex = nextLeafIndex;
        nextLeafIndex += 1;

        emit Deposit(commitment, leafIndex, block.timestamp);
    }

    // ──────────────────────────────────────────────────────────────────────
    //  Withdraw
    // ──────────────────────────────────────────────────────────────────────

    /// @notice Withdraw funds from the pool by revealing the preimage of a commitment
    /// @param secret     The 32-byte secret used to create the commitment
    /// @param nullifier  The 32-byte nullifier used to create the commitment
    /// @param recipient  The address to receive the withdrawn ETH
    /// @dev The caller (relayer) pays gas. The recipient receives DENOMINATION ETH.
    function withdraw(
        bytes32 secret,
        bytes32 nullifier,
        address payable recipient
    ) external {
        // Reconstruct the commitment from the provided preimage
        bytes32 commitment = sha256(abi.encodePacked(secret, nullifier));

        // Verify the commitment was previously deposited
        if (!commitments[commitment]) {
            revert CommitmentNotFound(commitment);
        }

        // Verify the nullifier hasn't been spent (prevents double-withdrawal)
        if (nullifiers[nullifier]) {
            revert NullifierAlreadySpent(nullifier);
        }

        // Mark the nullifier as spent
        nullifiers[nullifier] = true;

        // Transfer the fixed denomination to the recipient
        (bool success, ) = recipient.call{value: DENOMINATION}("");
        if (!success) {
            revert WithdrawTransferFailed();
        }

        emit Withdrawal(recipient, nullifier);
    }

    // ──────────────────────────────────────────────────────────────────────
    //  View Helpers
    // ──────────────────────────────────────────────────────────────────────

    /// @notice Check if a commitment exists in the pool
    function isCommitmentValid(bytes32 commitment) external view returns (bool) {
        return commitments[commitment];
    }

    /// @notice Check if a nullifier has already been spent
    function isNullifierSpent(bytes32 nullifier) external view returns (bool) {
        return nullifiers[nullifier];
    }

    /// @notice Returns the current pool balance
    function poolBalance() external view returns (uint256) {
        return address(this).balance;
    }

    /// @notice Accept ETH sent directly (e.g., from selfdestruct or coinbase)
    receive() external payable {}
}
