#!/bin/bash
set -e

# Default to uid/gid 1000 if not specified
PUID=${PUID:-1000}
PGID=${PGID:-1000}

echo "Starting Deduparr with UID:GID ${PUID}:${PGID}"

# Get current deduparr user's UID/GID
CURRENT_UID=$(id -u deduparr)
CURRENT_GID=$(id -g deduparr)

# Only modify if different from current
if [ "$PUID" != "$CURRENT_UID" ] || [ "$PGID" != "$CURRENT_GID" ]; then
    echo "Updating deduparr user to UID:GID ${PUID}:${PGID}"
    
    # Modify group first
    if [ "$PGID" != "$CURRENT_GID" ]; then
        groupmod -o -g "$PGID" deduparr
    fi
    
    # Modify user
    if [ "$PUID" != "$CURRENT_UID" ]; then
        usermod -o -u "$PUID" deduparr
    fi
    
    # Fix permissions on key directories
    echo "Updating permissions..."
    chown -R deduparr:deduparr \
        /app \
        /config \
        /home/deduparr \
        2>/dev/null || true
fi

# Execute the command (supervisord)
echo "Starting supervisord..."
exec "$@"
