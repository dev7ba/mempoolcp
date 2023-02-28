Mempoolcp
=========

What is mempoolcp?
------------------

Mempoolpc is a command-line program to copy the mempool from one bitcoin node to another.

How does it works?
------------------

Through bitcoin nodes rpc interface, this program uses `getrawmempool(verbose)`, `getmempoolentry`, `getrawtransaction` and `sendrawtransaction` rpc calls to copy the mempool between nodes. Be aware that both nodes must be configured to use user/password or cookie authentication for rpc calls in `bitcoin.conf`:

```sh
rpcbind=my_ip_address_here
rpcallowip=my_ip_address_here
rpcuser=myusername
rpcpasswd=mypassword
rpccookiefile=path_to_cookie_file
```

Mempoolpc takes into account the dependencies between transactions and the fact that you can't send a child tx before a parent tx, or a parent tx before a grandparent tx... because otherwise, the sent transactions could be denied by the receiving node.

Mempoolcp is fast, as fast as rust [serde](https://serde.rs) is. Also, mempoolcp use multithreading when possible.

It has two modes of operation: a faster one using more memory and a normal one using less. The faster uses getrawmempool_verbose (a heavy call that uses a lot of memory if there are many txs). and then getrawtransaction + sendrawTransaction for each transaction. The normal mode uses getrawmempool (without verbose), then getmempoolentry + getrawtransaction + sendrawTransaction for each transaction.

To reduce the final txs differences between mempools, a ZMQ subscriber listening at source node can be configured to store incoming txs while the program is executing. Before program ends, those txs are sent to destination node.

Configuration is done via the command line or via config file (to avoid using passwords in the shell).

It can actively ask for the rpc user and password if needed, or use cookie authentication.

It has an option to choose network (ports): mainnet, testnet, regtest...

It is compatible with any limitancestorcount value in bitcoin.conf

Usage
-----

Basic use (using default rpc ports) is as follows:

```sh
mempoolcp <SOURCE_IP_ADDR> <DEST_IP_ADDR>
```

Ports are defined via `--source-port` `-p` or `-dest-port` `-P`

```sh
mempoolcp <SOURCE_IP_ADDR> <DEST_IP_ADDR> --source-port 8332 --dest-port 8332
```

If you are using standard rpc ports in your bitcoind nodes you can use `--net` or `-t` `<NET>` [possible values: main-net, test-net, sig-net, reg-test] default: main-net

```sh
mempoolcp <SOURCE_IP_ADDR> <DEST_IP_ADDR> --net test-net
```

Normally source/destination users/passwords are asked by the command line.

If you want to set authorization data via command line you can use:

```sh
mempoolcp <SOURCE_IP_ADDR> <DEST_IP_ADDR> --source-user <SOURCE_USER> --source-passwd <SOURCE_PASSWD> --dest-user <DEST_USER> --dest-passwd <DEST_PASSWD>
```

but be aware of credentials leak via `history` command.

Do not forget to add `rpcuser=my_user_name` and `rpcpasswd=my_passwd` in `bitcoin.conf` file.

Another option is to use cookie authentication using `--source-cookie-auth-path` `-s` and `--dest-cookie-auth-path` `-d`

```sh
mempoolcp <SOURCE_IP_ADDR> <DEST_IP_ADDR> --source-cookie-auth-path <SOURCE_COOKIE_PATH> --dest-cookie-auth-path <DEST_COOKIE_PATH>
```

Do not forget to add `rpccookiefile=path_to_cookie_file` in `bitcoin.conf` file (Normally ``.cookie``).

Another option is to use the `--use-config` `-c` option to use a configuration file in `~/.config/mempoolcp/default-config.toml` with the following contents:

```sh
source_ip_addr = 'my_source_ip'
dest_ip_addr = 'my_dest_ip'
source_user = 'my_source_user'
source_passwd = 'my_source_passwd'
dest_user = 'my_dest_user'
dest_passwd = 'my_dest_user'
source_cookie_auth_path = 'path_to_cookie_file'
dest_cookie_auth_path = 'path_to_cookie_file'
net = 'MainNet'
zmq_address = 'tcp://my_zmq_addres:my_zmq_port'
fast_mode = false
verbose = false
```

```sh
mempoolcp . . --use-config
```

Note the use of '.' instead of source/dest ips. All configuration will be loaded from file.

If `~/.config/mempoolcp/default-config.toml` does not exist. It will be created with the current cmd params at invocation.  You can use other filepath using `--use-config-path` ```sh mempoolcp . . --use-config-path /my-path/my-file

If `/my-path/my-file` does not exist. It will be created with the current cmd params at invocation at `/my-path/my-file.toml` Do not write .toml extension in path, only filename.

By default, `mempoolcp` uses a normal mode-memory saving mode. To enable the fast mode-memory hungry mode use `-fast-mode` `-f`  

```sh
mempoolcp <SOURCE_IP_ADDR> <DEST_IP_ADDR> --fast-mode
```

ZMQ listening interface is optional, but if you want you enable to obtain better results use ``-z`` or `--zmq-address` in the command line or `zmq_address` in configuration file.

Be aware that as ZMQ subscribers do not need a running server to bind, if you misspell the address, the program will not end since is waiting on a unexisting server direction. Even so, URL sintax is checked.

Do not forget to add ``zmqpubrawtx=tcp://my_pub_ip:my_pub_port`` in source node `bitcoin.conf` file.

A `--verbose` `-v` mode exists for displaying additional data as: effective configuration, transaction dependencies histogram and failed rpc calls

You can see all options via `--help` or `-h` option

```sh
mempoolcp --help
```

[TANSTAAGM](https://lists.linuxfoundation.org/pipermail/bitcoin-dev/2020-July/018017.html) - There Ain't No Such Thing As A Global Mempool
---------------------------------------------------------

Be aware that it's really difficult to have two mempools with the same transaction set due to different peers connections, conflicting transactions or transaction arrival timing issues between nodes (among other issues).

Compilling instructions
-----------------------

- Install [rust](https://rustup.rs/) in your system
- Clone the repository in a directory: `git clone https://github.com/dev7ba/mempoolcp.git`
- Go into directory and execute `cargo build` or `cargo build --release`. The executable will appear in `/mempoolcp/target/debug` or in `/mempoolcp/target/release`
- Enjoy

