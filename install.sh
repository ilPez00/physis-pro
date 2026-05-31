#!/bin/bash
# Physis Pro Installation Script

INSTALL_DIR="/usr/local/bin/physis"
CONFIG_DIR="/etc/physis"

echo "Installing Physis Pro..."

# Create directories
sudo mkdir -p $INSTALL_DIR
sudo mkdir -p $CONFIG_DIR

# Copy binary and python gateway (assuming build is done)
sudo cp target/release/physis-web $INSTALL_DIR/
sudo cp -r gateway/ $INSTALL_DIR/
sudo cp -r config/ $CONFIG_DIR/

# Create a wrapper script
sudo tee /usr/local/bin/physis-pro <<EOF
#!/bin/bash
export PHYSIS_BIN="$INSTALL_DIR/physis-web"
python3 "$INSTALL_DIR/gateway/physis_mcp_gateway.py" "\$@"
EOF

sudo chmod +x /usr/local/bin/physis-pro

echo "Installation complete."
echo "You can now run 'physis-pro' from anywhere."
