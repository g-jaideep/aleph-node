#!/bin/bash

cd erc20/

# cargo +nightly contract build --release

cargo +nightly contract upload --suri 'bottom drive obey lake curtain smoke basket hold race lonely fit walk//Filip' --url 'wss://ws-smartnet.test.azero.dev:443'

# ws://ws-smartnet.test.azero.dev --verbose
# wss://ws-smartnet.test.azero.dev:443 --verbose

exit $?
