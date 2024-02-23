# CW404 Spec: Fractional Non Fungible Tokens

CW404 is a specification for fractional non-fungible tokens based on CosmWasm.
The name and design is based on the [ERC404 standard by the Pandora team](https://github.com/0xacme/ERC404),
with some enhancements.

The specification is split into multiple sections, a contract may only
implement some of this functionality, but must implement the base.

## Note

As much as possible, the ERC404 standard has been modified to be in-line with both CW20 and CW721 function signatures to optimise for usage. This is desirable as the naming on CW20 and CW721 are highly explicit as opposed to the ERC20 and ERC721 standards which are largely ambiguous. As an example:

```solidity
// ERC20 transferFrom
function transferFrom(address from, address to, uint256 value) public;

// ERC721 transferFrom
function transferFrom(address from, address to, uint256 tokenId) public;
```

Versus

```
// CW20
Transfer{recipient, amount}

// CW721 transfer
TransferNft{recipient, token_id}
```

The net-effect of the above is that, instead of worrying about understanding the underlying `transferFrom` source code, users need to only use `TransferNft{recipient, token_id}` for transferring NFTs and `Transfer{recipient, amount}` for transferring CW20 tokens.

## Rules of engagement

Before we begin, it is important to understand the rules of engagement of the 404 standard, so developers can plan around this to create unique mechanics:

- When you hold 1 CW404 token in your wallet, an NFT gets minted to your wallet
- Assuming you send out a fractional amount such that you now have less than 1 CW404 token in your wallet, an NFT gets burnt from your wallet
- The burn ordering follows a last-in-first-out (LIFO) ordering
- Token IDs that are burnt will not get recycled
- Assuming a max supply of 10k NFTs, it is possible to have Token IDs >= 10,000. However, because previous NFTs have been burnt, max supply is still 10k

Whitelist mechanics:
- There's a whitelist feature to allow saving of gas for _core_ contracts/addresses
- Whitelisted contracts/addresses will not have NFTs minted into said wallet when transfer of full CW404 tokens are sent in (saves gas)
- Whitelisted contracts *will mint, but will not burn NFTs* from the address when CW404 tokens are sent out

Lock feature:
- There's a "lock" feature included in the contract to allow users to lock up token IDs for art that they potentially really love, and do not wish to potentially fat-finger burn them
- It will result in transaction reversions when a NFT is about to be burnt within a transaction, thus nullifying any potential burns

### Messages

`TransferNft{recipient, token_id}` -
This transfers ownership of the token to `recipient` account. This is
designed to send to an address controlled by a private key and _does not_
trigger any actions on the recipient if it is a contract.

Requires `token_id` to point to a valid token, and `env.sender` to be
the owner of it, or have an allowance to transfer it.

`SendNft{contract, token_id, msg}` -
This transfers ownership of the token to `contract` account. `contract`
must be an address controlled by a smart contract, which implements
the CW721Receiver interface. The `msg` will be passed to the recipient
contract, along with the token_id.

Requires `token_id` to point to a valid token, and `env.sender` to be
the owner of it, or have an allowance to transfer it.

`Approve{spender, token_id, expires}` - Grants permission to `spender` to
transfer or send the given token. This can only be performed when
`env.sender` is the owner of the given `token_id` or an `operator`.
There can be multiple spender accounts per token, and they are cleared once
the token is transferred or sent.

`Revoke{spender, token_id}` - This revokes a previously granted permission
to transfer the given `token_id`. This can only be granted when
`env.sender` is the owner of the given `token_id` or an `operator`.

`ApproveAll{operator, expires}` - Grant `operator` permission to transfer or send
all tokens owned by `env.sender`. This approval is tied to the owner, not the
tokens and applies to any future token that the owner receives as well.

`RevokeAll{operator}` - Revoke a previous `ApproveAll` permission granted
to the given `operator`.

`Transfer{recipient, amount}` - Moves `amount` CW20 tokens from the `info.sender` account to the `recipient` account. This is designed to send to an address controlled by a private key and does not trigger any actions on the recipient if it is a contract.

`Send{contract, amount, msg}` - Moves `amount` CW20 tokens from the `info.sender` account to the `contract` account. `contract` must be an address of a contract that implements the `Receiver` interface. The msg will be passed to the recipient contract, along with the amount.

### Queries

`OwnerOf{token_id, include_expired}` - Returns the owner of the given token,
as well as anyone with approval on this particular token. If the token is
unknown, returns an error. Return type is `OwnerOfResponse`. If
`include_expired` is set, show expired owners in the results, otherwise, ignore
them.

`AllOperators{owner, include_expired, start_after, limit}` - List all
operators that can access all of the owner's tokens. Return type is
`OperatorsResponse`. If `include_expired` is set, show expired owners in the
results, otherwise, ignore them. If `start_after` is set, then it returns the
first `limit` operators _after_ the given one.

`NumTokens{}` - Total number of tokens issued

### Receiver

The counter-part to `SendNft` is `ReceiveNft`, which must be implemented by
any contract that wishes to manage CW721 tokens. This is generally _not_
implemented by any CW721 contract.

`ReceiveNft{sender, token_id, msg}` - This is designed to handle `SendNft`
messages. The address of the contract is stored in `env.sender`
so it cannot be faked. The contract should ensure the sender matches
the token contract it expects to handle, and not allow arbitrary addresses.

The `sender` is the original account requesting to move the token
and `msg` is a `Binary` data that can be decoded into a contract-specific
message. This can be empty if we have only one default action,
or it may be a `ReceiveMsg` variant to clarify the intention. For example,
if I send to an exchange, I can specify the price I want to list the token
for.

## Metadata

### Queries

`ContractInfo{}` - This returns top-level metadata about the contract.
Namely, `name` and `symbol`.

`NftInfo{token_id}` - This returns metadata about one particular token.
The return value is based on _ERC721 Metadata JSON Schema_, but directly
from the contract, not as a Uri. Only the image link is a Uri.

`AllNftInfo{token_id}` - This returns the result of both `NftInfo`
and `OwnerOf` as one query as an optimization for clients, which may
want both info to display one NFT.

## Enumerable

### Queries

Pagination is achieved via `start_after` and `limit`. Limit is a request
set by the client, if unset, the contract will automatically set it to
`DefaultLimit` (suggested 10). If set, it will be used up to a `MaxLimit`
value (suggested 30). Contracts can define other `DefaultLimit` and `MaxLimit`
values without violating the CW721 spec, and clients should not rely on
any particular values.

If `start_after` is unset, the query returns the first results, ordered
lexicographically by `token_id`. If `start_after` is set, then it returns the
first `limit` tokens _after_ the given one. This allows straightforward
pagination by taking the last result returned (a `token_id`) and using it
as the `start_after` value in a future query.

`Tokens{owner, start_after, limit}` - List all token_ids that belong to a given owner.
Return type is `TokensResponse{tokens: Vec<token_id>}`.

`AllTokens{start_after, limit}` - Requires pagination. Lists all token_ids controlled by
the contract.
