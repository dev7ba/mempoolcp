# mempoolcp

 Mempoolpc is a command-line program to copy the mempool of one bitcoin node to another.

    Mempoolpc takes into account the dependencies between transactions and the fact that you can't send a child before a parent, or a parent before a grandparent... because otherwise, the sent transactions could be denied.
    It has two modes of operation: a faster one using more memory and another slower one using less.
    The faster one uses getrawmempool_verbose (a heavy call that uses a lot of memory if there are many txs). and then getrawtransaction + sendrawTransaction for each transaction.
    The slower mode uses getrawmempool (without verbose), then getmempoolentry + getrawtransaction + sendrawTransaction for each transaction.
    Configuration is done via the command line (Reckless mode) or via mempoolcp.conf in a file (to avoid putting passw in the shell). It can also actively ask for the user and password.
    In the future, it should also take into account transactions that have arrived at the source node via ZMQ while it was performing the operation. (If a transaction arrives at the started node that doesn't have parents yet, it will be denied, but if it is queued until the copy operation is finished and then sent, it will be accepted by the destination node).
    It has an option to choose network (ports): mainnet, testnet, regtest...
    Nothing of this is implemented yet...
