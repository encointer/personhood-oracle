# Privacy-Preserving Personhood Oracle

## Run Locally in SW Mode in Docker

clone the repos needed into the same top folder:
```
git clone https://github.com/encointer/personhood-oracle.git
git clone https://github.com/encointer/encointer-node.git
git clone https://github.com/integritee-network/integritee-node.git
```

build Encointer node

```
cd encointer-node
git checkout polkadot-v0.9.42
cargo build --release
cp target/release/encointer-node-notee ../personhood-oracle/bin/
```

build Integritee node

```
cd integritee-node
cargo build --release
cp target/release/integritee-node ../personhood-oracle/bin/
```

setup development environment in docker:

```
cd personhood-oracle
docker run --name integritee-dev-personhood-and-nodes -it -p 9944:9944 -p 9966:9966 -p 9988:9988 -v $(pwd):/home/ubuntu/personhood-oracle -v $(pwd)/../encointer-node:/home/ubuntu/encointer-node -e MYUID=$(id -u) -e MYGUID=$(id -g) integritee/integritee-dev:0.2.2 /bin/bash
```

build personhood oracle (inside docker)
```
cd personhood-oracle
SGX_MODE=SW WORKER_FEATURES= WORKER_MODE=teeracle make
```

### Run the Demo

(re)enter the docker container created above
```
docker start -a -i integritee-dev-personhood-and-nodes
```
use tmux to split into 3 panes

If you want to use our launch script, use

```
./local-setup/launch.py ./local-setup/config/personhood-oracle.json
```
Logs will be piped to `./log/*` and you could watch them using tmux:
```
cd local-setup && ./tmux_logger_two_nodes.sh
```
otherwise, you can start everything manually:

run blockchain node
```
cd ~/encointer-node
./target/release/encointer-node-notee --dev --enable-offchain-indexing true --ws-external --rpc-cors all
```
the port 9944 will be mapped to the host, so you can observe what's happening

run community simulation (on host)
```
encointer-node/client/bootstrap_demo_community.py -p 9966
```
wait a few minutes until bootstrapping has completed

run oracle (inside docker)
```
cd ~/personhood-oracle/bin
export RUST_LOG=info,substrate_api_client=warn,ws=warn,mio=warn,its_consensus_common=info,sidechain=info,integritee_service=trace,enclave_runtime=trace,ac_node_api=warn,sp_io=warn,itc_parentchain_indirect_calls_executor=trace,itp_stf_executor=trace,itc_parentchain_light_client=trace,itc_parentchain_block_importer=trace,itp_stf_state_handler=trace,itc_direct_rpc_server=trace
./integritee-service -c run --skip-ra --dev
```
in the best case, wait a few minutes until you see a teerex.registerSgxEnclave Event on the blockchain. Not strictly necessary

#### claim nostr badge 

create demo account for Alice:

https://iris.to/settings

we created this one: `npub1xq33nsus0d2d00jzdea4unl08p35cd05md7mmgtxky5sncsjgxvqw2p77y`

```
cd ~/personhood-oracle/bin
./integritee-cli -p 9966 personhood-oracle issue-nostr-badge //Alice npub1xq33nsus0d2d00jzdea4unl08p35cd05md7mmgtxky5sncsjgxvqw2p77y sqm1v79dF6b wss://relay.damus.io
```
you should see

```
Nostr badge has been issued successfully.
badge award note id: note1nzlnzyjuuljltrr4dqkdfaqtnjq6jyzvpev5wugvtltfwud6zulq0r0cg2
```
you can now check the badge by visiting
https://snort.social/p/npub1xq33nsus0d2d00jzdea4unl08p35cd05md7mmgtxky5sncsjgxvqw2p77y

you may need to make sure to subscribe to relay.damus.io in settings

find the badge definition published here:
https://badges.page/b/naddr1qqx8qetjwdhku6r0daj97vgzypz2eydm7k6h8cs4wf9n5ylwux8vatzc9sdhjqnw02nnnx7kkuvluqcyqqq82wgtjchvs
