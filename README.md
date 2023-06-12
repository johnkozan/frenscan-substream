# Frenscan Substream
[![License](https://img.shields.io/badge/License-Apache%202.0-blue.svg)](https://opensource.org/licenses/Apache-2.0)

A DAO focused blockchain explorer, built on [StreamingFast Substreams](https://substreams.streamingfast.io/).


## Install Dependencies

1. [Install `substreams` cli](https://substreams.streamingfast.io/getting-started/installing-the-cli)
2. [Install postgres sink](https://substreams.streamingfast.io/developers-guide/sink-targets/substreams-sink-postgres)
3. Optionally install Docker and `docker-compose` to run a PostresQL database container.

## Quick Start

1. [Obtain a StreamingFast API Key](https://substreams.streamingfast.io/reference-and-specs/authentication) and set the `SUBSTREAMS_API_TOKEN`
   with the `sftoken` command.
2. Create a `frens.yaml` file to define the DAO to be indexed, or copy one from the `examples/` directory.  See `frens.yaml` section for specification.
```bash
cp examples/citydao.yaml ./frens.yaml
```
3. Start the database:
```bash
docker-compose up
```
4. Build the substream:
```bash
make build
```
5. Prepare the database:
```bash
make setup_postgres
```
6. Start indexing:
```bash
make sink_postgres
```


## frens.yaml

**WARNING** This project is a work in progress and the file format is subject to change


`frens.yaml` is a file which enumerates all of the Ethereum accounts belonging to a dApp or DAO.
The file has two main sections: `treasury_accounts` and `tokens_issued`.  


```yaml
---
version: 0.1.0                 # frens.yaml specification version
name: DAO Name
treasury_accounts:
  - name: Treasury account
    address: 0x........
    network: mainnet           # Optional, defaults to mainnet
tokens_issued:
  - name: DAO Token
    address: 0x.......
    network: mainnet           # Optional, defaults to mainnet
    schema: erc20              # Supported: erc20, erc721, erc1155


```

## Issues / Current limitations

* This substream was written before StreamingFast released the [ETH Balance changes substream](https://github.com/streamingfast/substreams-eth-balance-changes)
  and [ERC20 Balance change substream](https://streamingfastio.medium.com/erc-20-balance-changes-substreams-73f1b6730c80).
  This substream will be re-written to take adavantage of these.
* There is no UI for the substream data at this time.
* Only one network is currently supported
* Roll-back of forked blocks in not currently supported.  The postgres-sink tails the chain head by 12 blocks in order to avoid forks.


## License

Apache
