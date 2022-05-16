import { RedspotUserConfig } from "redspot/types";
import "@redspot/patract";
import "@redspot/chai";
import "@redspot/gas-reporter";
import "@redspot/known-types";
import "@redspot/watcher";
import "@redspot/explorer";
import "@redspot/decimals";
import "dotenv/config"; // Store environment-specific variable from '.env' to process.env

// console.log(process.env.SMARTNET_MNEMONIC)

export default {
  defaultNetwork: "development",
  contract: {
    ink: {
      docker: false,
      toolchain: "nightly",
      sources: ["./**/*", '!./trait-erc20/*'],
    },
  },
  networks: {
    development: {
      endpoint: "ws://127.0.0.1:9944",
      gasLimit: "400000000000",
      types: {},
    },
    smartnet: {
      endpoint: "wss://ws-smartnet.test.azero.dev",
      gasLimit: "1048576",
      accounts: [process.env.SMARTNET_MNEMONIC],
      types: {},
    },
  },
  mocha: {
    timeout: 60000,
  },
} as RedspotUserConfig;
