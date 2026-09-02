/// SPDX-License-Identifier: Apache-2.0

contract increment_with_pause {
    address private instance pauseAddr;
    uint32 private instance count;

    error PausedError();

    constructor(address _pauseAddr) {
        pauseAddr = _pauseAddr;
    }

    function increment() public returns (uint32) {
        bytes memory payload = abi.encode("paused");
        (bool ok, bytes memory ret) = pauseAddr.call(payload);
        
        // Decode the return value if the call succeeds
        if (ok) {
            bool isPaused = abi.decode(ret, (bool));
            if (isPaused) {
                revert PausedError();
            }
        }

        count += 1;
        return count;
    }
}

contract pause {
    bool private instance _isPaused;

    function paused() public view returns (bool) {
        return _isPaused;
    }

    function set(bool p) public {
        _isPaused = p;
    }
}