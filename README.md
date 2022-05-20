# archway-liquid-staking

Project Archway Liquid Staking 

## Contracts:

- **Staking**: for staking, unstaking and claiming unstaked tokens. On staking request, the native token are converted to liquid token which start accruing returns. Also unstaking operation initiates a claiming delay of maximum 21 days (unbonding duration), after which the native token can be redeemed back using claim unbonded tokens action.

- **Swap**: for providing liquidity and swapping. On providing liquidity operation, the native token are converted to derivative token which start accruing returns. On swapping request, the liquid token are converted to native token (reduced by some percentage as swap fee).

- **Liquid Token**: using cw20-base contract, this is a representation of staked native token. The owner of the tokens continuously accrues returns on the liquid token kept.

## Flows:

![Contract flows](docs/contract-flows.png)