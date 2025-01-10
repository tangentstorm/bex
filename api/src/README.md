# Bex API

This program provides a simple HTTP API for constructing
binary decision diagrams using bex. Below are the steps
to set up and use the API.

## Setup

0. Install Rust:

    Follow the instructions at [rust-lang.org](https://www.rust-lang.org/tools/install) to install Rust and the `cargo` build system.

1. Clone the repository:
    ```sh
    git clone https://github.com/tangentstorm/bex.git
    cd bex
    ```

2. Navigate to the API directory:
    ```sh
    cd api
    ```

3. (Optional) Create a `.env` file in the `api` directory with the following content:
    ```
    HOST=127.0.0.1
    PORT=3030
    ```

    If the `.env` file is not created, the default values will be used (`HOST=127.0.0.1` and `PORT=3030`).

4. Build and run the API:
    ```sh
    cargo run
    ```

## Usage

### Endpoints

The following endpoints are available to interact with the BDD base:

| Endpoint | Description |
| --- | --- |
| **GET /ite/{vid}/{nid1}/{nid2}** | Build an ITE (If-Then-Else) node for the given NIDs. |
| **GET /nid/{nid}** | Retrieve the high and low branches of the given NID. |
| **GET /xor/{nid1}/{nid2}** | Perform XOR operation on the given NIDs. |
| **GET /and/{nid1}/{nid2}** | Perform AND operation on the given NIDs. |
| **GET /or/{nid1}/{nid2}** | Perform OR operation on the given NIDs. |

### Example Usage

1. Build an ITE node:
    ```
    http://localhost:3030/ite/x1/x2/x3
    ```

2. Inspect the resulting node:
    ```
    http://localhost:3030/nid/x3.2
    ```

3. Perform an XOR operation:
    ```
    http://localhost:3030/xor/x1/x2
    ```

4. Perform an AND operation:
    ```
    http://localhost:3030/and/x1/x2
    ```

5. Perform an OR operation:
    ```
    http://localhost:3030/or/x1/x2
    ```

## License

This project is licensed under the MIT License.
