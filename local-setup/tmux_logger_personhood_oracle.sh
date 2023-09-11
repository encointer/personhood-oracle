#!/bin/bash

# script that setups a tmux session with three panes that attach to the log files
# of the node and the two workers launched by `./launch.py`

#################################################################################
# If you work with docker:
#
# 1.  run: ./launch.py in docker
# 2.  open a new bash session in a new window in the running container:
#     docker exec -it [container-id] bash
# 3.  run this script: ./tmux_logger.sh
#################################################################################

export TERM=xterm-color

if tmux has-session -t personhood_oracle_demo ; then
  echo "detected existing polkadot logger session, attaching..."
else
  # or start it up freshly
  tmux new-session -d -s personhood_oracle_demo \; \
    split-window -h \; \
    split-window -v \; \
    split-window -v \; \
    split-window -v \; \
    select-layout main-vertical \; \
    set -g pane-border-status top \; \
    send-keys -t personhood_oracle_demo:0.0 "printf '\033]2;USER\033\\'; clear" C-m \; \
    send-keys -t personhood_oracle_demo:0.1 "printf '\033]2;INTEGRITEE NODE\033\\'; clear" C-m \; \
    send-keys -t personhood_oracle_demo:0.1 'tail -f ../log/node1.log' C-m \; \
    send-keys -t personhood_oracle_demo:0.2 "printf '\033]2;ENCOINTER NODE\033\\'; clear" C-m \; \
    send-keys -t personhood_oracle_demo:0.2 'tail -f ../log/node2.log' C-m \; \
    send-keys -t personhood_oracle_demo:0.3 "printf '\033]2;SUBSTRATE NODE TEMPLATE\033\\'; clear" C-m \; \
    send-keys -t personhood_oracle_demo:0.3 'tail -f ../log/node3.log' C-m \; \
    send-keys -t personhood_oracle_demo:0.4 "printf '\033]2;TEE PERSONHOOD ORACLE\033\\'; clear" C-m \; \
    send-keys -t personhood_oracle_demo:0.4 'tail -f ../log/worker1.log' C-m \;

    # Attention: Depending on your tmux conf, indexes may start at 1
    tmux setw -g mouse on
fi
tmux attach-session -d -t personhood_oracle_demo