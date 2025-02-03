#!/bin/sh

# Ensure all required environment variables are set
: "${SERVER_NAME:?Environment variable SERVER_NAME is required}"
: "${SERVER_ADDRESS:?Environment variable SERVER_ADDRESS is required}"
: "${HTTP_ADDRESS:?Environment variable HTTP_ADDRESS is required}"
: "${JS_DOMAIN:?Environment variable JS_DOMAIN is required}"
: "${STORE_PATH:?Environment variable STORE_PATH is required}"
: "${CLUSTER_PORT:?Environment variable CLUSTER_PORT is required}"
: "${CLUSTER_SEED_ADDRESSES:?Environment variable CLUSTER_SEED_ADDRESSES is required}"
: "${CLUSTER_USER_NAME:?Environment variable CLUSTER_USER_NAME is required}"
: "${CLUSTER_USER_PW:?Environment variable CLUSTER_USER_PW is required}"
: "${RESOLVER_PATH:?Environment variable RESOLVER_PATH is required}"

# Define the output config file
CONFIG_FILE="nats-cluster-server.conf"

# Create the configuration file
cat > "$CONFIG_FILE" <<EOL
# Cluster Node Configuration
server_name: ${SERVER_NAME}
listen: ${SERVER_ADDRESS}
http: ${HTTP_ADDRESS}

system_account: SYS

jetstream: {
  enabled: true
  domain: ${JS_DOMAIN}
  store_dir: "${STORE_PATH}/${JS_DOMAIN}"
}

# Cluster configuration
cluster {
  name: nats_cluster
  listen: 0.0.0.0:${CLUSTER_PORT}

  # Route to connect to the seed server(s)
  routes: [
    ${CLUSTER_SEED_ADDRESSES}
  ]

  authorization {
    user: ${CLUSTER_USER_NAME}
    password: ${CLUSTER_USER_PW}
  }
}

# Leaf node connection
leafnodes {
  port: 7422
}

include ${RESOLVER_PATH}

# logging options
debug:   true
trace:   true
logtime: false

# max_connections
max_connections: 100

# max_subscriptions (per connection)
max_subscriptions: 1000

# max_pending
max_pending: 10000000

# maximum control line
max_control_line: 2048

# maximum payload
max_payload: 65536

# ping interval and no pong threshold
ping_interval: "60s"
ping_max: 3

# how long server can block on a socket write to a client
write_deadline: "3s"

lame_duck_duration: "4m"

# report repeated failed route/gateway/leafNode connection
# every 24hour (24*60*60)
connect_error_reports: 86400

# report failed reconnect events every 5 attempts
reconnect_error_reports: 5
EOL

echo "NATS configuration file '$CONFIG_FILE' has been created successfully."
