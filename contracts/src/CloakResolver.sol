// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

/// @title CloakResolver - A CCIP-Read (EIP-3668) enabled ENS Resolver for Stealth Addressing
/// @notice This contract throws an OffchainLookup error, directing the client (e.g., MetaMask, ethers.js)
///         to ping the CloakFund Rust backend. The backend dynamically generates a brand-new
///         EIP-5564 stealth address, signs it, and returns it to the client for trustless verification.
/// @dev Implements a subset of ENS resolution (addr) using EIP-3668 off-chain gateways.

interface IENS {
    function owner(bytes32 node) external view returns (address);
}

interface IResolverService {
    function resolve(bytes calldata name, bytes calldata data) external view returns (bytes memory result, uint64 expires, bytes memory sig);
}

contract CloakResolver {
    /// @dev EIP-3668 Standard Error. When a wallet sees this, it stops the on-chain tx,
    /// pings the `url`, gets the payload, and calls `callbackFunction` with the result.
    error OffchainLookup(
        address sender,
        string[] urls,
        bytes callData,
        bytes4 callbackFunction,
        bytes extraData
    );

    /// @notice The URL of the CloakFund Rust Backend CCIP-Read API
    string[] public gatewayUrls;

    /// @notice The public key used to verify the Rust backend's signature
    address public signer;

    /// @notice Address of the ENS Registry
    IENS public immutable ens;

    event GatewayUpdated(string[] urls);
    event SignerUpdated(address newSigner);

    constructor(string[] memory _urls, address _signer, address _ens) {
        gatewayUrls = _urls;
        signer = _signer;
        ens = IENS(_ens);
    }

    /// @notice Allows the owner to update the Rust backend API endpoints
    function setGateways(string[] calldata _urls) external {
        // In a production contract, add an onlyOwner modifier here
        gatewayUrls = _urls;
        emit GatewayUpdated(_urls);
    }

    /// @notice Updates the trusted signer that verifies stealth addresses generated off-chain
    function setSigner(address _signer) external {
        // In a production contract, add an onlyOwner modifier here
        signer = _signer;
        emit SignerUpdated(_signer);
    }

    /// @notice Core ENS resolution function for `addr(bytes32)`
    /// @dev Instead of returning a static address, it forces the wallet to call the Rust backend.
    function resolve(bytes calldata name, bytes calldata data) external view returns (bytes memory) {
        // 1. We construct the OffchainLookup error defined in EIP-3668.
        // 2. We pass the requested ENS `name` and the `data` (which contains the `addr` function selector).
        // 3. The wallet will ping `gatewayUrls[0]/{sender}/{data}`.
        // 4. The backend will return the dynamically generated stealth address and a signature.
        // 5. The wallet will then automatically call `resolveWithProof` below.

        revert OffchainLookup(
            address(this),
            gatewayUrls,
            abi.encodeCall(IResolverService.resolve, (name, data)),
            this.resolveWithProof.selector,
            abi.encode(name, data) // Extra data passed back to verify the specific request
        );
    }

    /// @notice The callback function called by the wallet after it gets the stealth address from the Rust backend
    /// @param response The ABI encoded stealth address and signature returned by the off-chain gateway
    /// @param extraData The original request data passed in the OffchainLookup error
    function resolveWithProof(bytes calldata response, bytes calldata extraData) external view returns (bytes memory) {
        // Decode the Rust backend's response
        (bytes memory result, uint64 expires, bytes memory sig) = abi.decode(response, (bytes, uint64, bytes));

        // Decode the original request to ensure it matches
        (bytes memory name, bytes memory data) = abi.decode(extraData, (bytes, bytes));

        // Security Check 1: Ensure the off-chain response hasn't expired
        require(block.timestamp <= expires, "CloakResolver: Signature expired");

        // Security Check 2: Verify the signature
        // We hash the payload just like the Rust backend did: Keccak256(resolverAddress, expires, request, result)
        bytes32 messageHash = keccak256(abi.encodePacked(
            "\x19Ethereum Signed Message:\n32",
            keccak256(abi.encodePacked(address(this), expires, abi.encodeCall(IResolverService.resolve, (name, data)), result))
        ));

        // Recover the signer from the signature
        require(recoverSigner(messageHash, sig) == signer, "CloakResolver: Invalid signature from gateway");

        // If the signature is valid, the wallet accepts this dynamic stealth address as the official ENS resolution!
        return result;
    }

    /// @dev Helper function to recover an ECDSA signer from a signature
    function recoverSigner(bytes32 _ethSignedMessageHash, bytes memory _signature) internal pure returns (address) {
        require(_signature.length == 65, "CloakResolver: Invalid signature length");
        bytes32 r;
        bytes32 s;
        uint8 v;
        assembly {
            r := mload(add(_signature, 32))
            s := mload(add(_signature, 64))
            v := byte(0, mload(add(_signature, 96)))
        }
        return ecrecover(_ethSignedMessageHash, v, r, s);
    }

    /// @notice Fallback to support EIP-165 interface detection
    function supportsInterface(bytes4 interfaceID) public pure returns (bool) {
        return interfaceID == type(IResolverService).interfaceId || interfaceID == 0x01ffc9a7; // ERC165
    }
}
