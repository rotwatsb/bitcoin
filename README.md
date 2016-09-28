<h3>A rust-written daemon for tracking recently mined blocks on the bitcoin network</h3>

<p>This project is (a start at) achieving a compromise in resource requirements for listen in on the bitcoin network, falling somwhere between the minimalism of SPV clients, where virtually no transaction data is stored, and the disk-space intensive needs of a full node that maintains a complete transaction history (currently ~90GB).</p>

<p>Instead, it listens to the network and tracks only the 1000 most recently mined blocks, saving both block heads and transactions to a PostgreSQL database as they come in. It's being written with the intention of serving as part of the backend to a lightweight block explorer. That project can be seen in its current state at <a href="www.rotwatsb.net/talk">www.rotwatsb.net/talk</a></p>

<p>The code uses the <a href="https://github.com/apoelstra/rust-bitcoin">Rust-Bitcoin library</a> to send, receive, and de)serialize network messages, and to maintain block data. It uses <a href="https://github.com/sfackler/rust-postgres">the Rust-Postgres crate</a> to write/read to/from the database.</p>

<p>If you want to run this yourself, you'll have to setup a postgres database and specify the connection details in the connection string specified in main.rs. You'll also need to initially supply the ip address of a first peer to connect to. If you're already running an spv client locally, connect to yourself at '127.0.0.1'. Then just 'cargo run' to start receiving network messages. The blockchain will be saved wherever you specify in the configuration string in main.rs</p>

<p>Also, it might be helpful to note that the current database schema is set up to be compatible with the <a href="https://github.com/rotwatsb/talk/blob/master/models.py">django models</a> being used in the related block explorer and discussion app.</p>