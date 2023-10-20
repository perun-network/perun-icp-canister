#!/bin/sh

startDFX() {
    path=$(which dfx)
    if [ -z "$path" ]; then
        echo "Error: dfx not found in PATH"
        return 1
    fi
    execPath="./userdata"
    cd $execPath
    $path start --background --clean --host 127.0.0.1:4943 &
    status=$?

    # Sleep to allow process to start
    sleep 3

    if [ $status -ne 0 ]; then
        echo "Error starting DFX..."
        return $status
    else
        echo "Starting DFX..."
        return $!
    fi
}

# Call the function
startDFX

