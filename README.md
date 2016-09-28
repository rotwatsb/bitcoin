<h3>A rust-written daemon for tracking recently mined blocks on the bitcoin network</h3>

<p>This project is (a start at) achieving a compromise in resource requirements for listening in on the bitcoin network, falling somwhere between the minimalism of SPV clients, where virtually no transaction data is stored, and the disk-space intensive needs of a full node that maintains a complete transaction history (currently ~90GB).</p>

<p>And while Bitcoin Core does offer an option to only store X MB of block data, no transaction data will be kept if this option is used.</p>

<p>Instead, this project listens to the network and tracks the 1000 most recently mined blocks, saving both block headers and transaction data to a PostgreSQL database as those blocks are mined. It's being written with the intention of serving as part of the backend to a lightweight block explorer. That project can be seen in its current state at [www.rotwatsb.net/talk](https://www.rotwatsb.net/talk)</p>

<p>The code uses the <a href="https://github.com/apoelstra/rust-bitcoin">Rust-Bitcoin library</a> to send, receive, and (de)serialize network messages, and to maintain block data. It uses <a href="https://github.com/sfackler/rust-postgres">the Rust-Postgres crate</a> to write/read to/from the database.</p>

<p>If you want to run this yourself, you'll have to setup a postgres database and specify the connection details in a connection string in main.rs. You'll also need to supply the ip address of an initial peer to first connect to. If you're already running an spv client locally, connect to yourself at '127.0.0.1'. Then just 'cargo run' to start receiving network messages. The blockchain will be saved wherever you specify in the configuration string in main.rs</p>

<p>Also, it might be helpful to note that the current database structure is set up to be compatible with the <a href="https://github.com/rotwatsb/talk/blob/master/models.py">django models</a> being used in a related block explorer and discussion app.</p>