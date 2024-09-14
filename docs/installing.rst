Installing Solang
=================

The Solang compiler is a single binary. It can be installed in different ways, listed below.

1. :ref:`Download from Homebrew <download-brew>` (MacOS only)
2. :ref:`Download binaries <download-binaries>`
3. :ref:`Download from a Docker container <download-docker>`
4. :ref:`Build using Dockerfile <build-dockerfile>`
5. :ref:`Build from source <build-source>`

.. _download-brew:

Option 1: Download from Brew
----------------------------

Solang is available on Brew via a private tap. This works only for MacOS systems, both Intel and Apple Silicon.
To install Solang via Brew, run the following command:

.. code-block:: text

    brew install hyperledger/solang/solang

.. _download-binaries:

Option 2: Download binaries
---------------------------

There are binaries available on github releases:

- `Linux x86-64 <https://github.com/hyperledger/solang/releases/download/v0.3.4/solang-linux-x86-64>`_
- `Linux arm64 <https://github.com/hyperledger/solang/releases/download/v0.3.4/solang-linux-arm64>`_
- `Windows x64 <https://github.com/hyperledger/solang/releases/download/v0.3.4/solang.exe>`_
- `MacOS intel <https://github.com/hyperledger/solang/releases/download/v0.3.4/solang-mac-intel>`_
- `MacOS arm <https://github.com/hyperledger/solang/releases/download/v0.3.4/solang-mac-arm>`_

Download the file and save it somewhere in your ``$PATH``, for example the bin directory in your home directory. If the
path you use is not already in ``$PATH``, then you need to add it yourself.

On MacOS, remember to give execution permission to the file and remove it from quarantine by executing the following commands:

.. code-block:: bash

    chmod +x solang-mac-arm
    xattr -d com.apple.quarantine solang-mac-arm

If you are using an Intel based Mac, please, exchange ``solang-mac-arm`` by ``solang-mac-intel`` in both of the above commands.

On Linux, permission to execute the binary is also necessary, so, please, run ``chmod +x solang-linux-x86-64``. If you
are using an Arm based Linux, the command is the following: ``chmod +x solang-linux-arm64``.

.. _download-docker:

Option 3: Use ghcr.io/hyperledger/solang containers
---------------------------------------------------

New images are automatically made available on
`solang containers <https://github.com/hyperledger/solang/pkgs/container/solang>`_.
There is a release `v0.3.4` tag and a `latest` tag:

.. code-block:: bash

	docker pull ghcr.io/hyperledger/solang:latest

The Solang binary is stored at ``/usr/bin/solang`` in this image. The `latest` tag
gets updated each time there is a commit to the main branch of the Solang git repository.

.. _build-dockerfile:

Option 4: Build Solang using Dockerfile
---------------------------------------

First clone the git repo using:

.. code-block:: bash

  git clone https://github.com/hyperledger/solang

Then you can build the image using:

.. code-block:: bash

	docker image build .

.. _build-source:

Option 5: Build Solang from source
----------------------------------

In order to build Solang from source, you will need:

* Rust version 1.74.0 or higher
* A C++ compiler with support for C++17
* A build of LLVM based on the Solana LLVM tree. There are a few LLVM patches required that are not upstream yet.

First, follow the steps below for installing LLVM and then proceed from there.

If you do not have the correct version of rust installed, go to `rustup <https://rustup.rs/>`_.
Compatible C++17 compilers are recent releases of ``gcc``, ``clang`` and Visual Studio 2019 or later.

To install Solang from sources, do the following:

1. :ref:`Install LLVM <install-llvm>` from Solana's LLVM fork.
2. :ref:`Build Solang <build-from-source>` from its source files.


Solang is also available on `crates.io`, so after completing step #1 from above, it is possible to :ref:`build it using the
release <build-from-crates>` on crates.

.. _install-llvm:

Step 1: Install the LLVM Libraries
_____________________________________

Solang needs a build of
`LLVM with some extra patches <https://github.com/solana-labs/llvm-project/>`_.
These patches make it possible to generate code for Solana, and fixes
concurrency issues in the lld linker.

You can either download the pre-built libraries from
`github <https://github.com/hyperledger/solang-llvm/releases>`_
or :ref:`build your own from source <llvm-from-source>`. After that, you need to add the ``bin`` of your
LLVM directory to your path, so that the build system of Solang can find the correct version of LLVM to use.

Linux
~~~~~

A pre-built version of LLVM, specifically configured for Solang, is available at
`<https://github.com/hyperledger/solang-llvm/releases/download/llvm16-0/llvm16.0-linux-x86-64.tar.xz>`_ for x86 processors
and at `<https://github.com/hyperledger/solang-llvm/releases/download/llvm16-0/llvm16.0-linux-arm64.tar.xz>`_ for ARM.
After downloading, untar the file in a terminal and add it to your path.

.. code-block:: bash

	tar Jxf llvm16.0-linux-x86-64.tar.xz
	export PATH=$(pwd)/llvm16.0/bin:$PATH

Windows
~~~~~~~

A pre-built version of LLVM, specifically configured for Solang, is available at
`<https://github.com/hyperledger/solang-llvm/releases/download/llvm16-0/llvm16.0-win.zip>`_.

After unzipping the file, add the bin directory to your path.

.. code-block:: batch

	set PATH=%PATH%;C:\llvm16.0\bin

Mac
~~~

A pre-built version of LLVM for intel macs, is available at
`<https://github.com/hyperledger/solang-llvm/releases/download/llvm16-0/llvm16.0-mac-intel.tar.xz>`_ and for arm macs there is
`<https://github.com/hyperledger/solang-llvm/releases/download/llvm16-0/llvm16.0-mac-arm.tar.xz>`_. After downloading,
untar the file in a terminal and add it to your path like so:

.. code-block:: bash

	tar Jxf llvm16.0-mac-arm.tar.xz
	xattr -rd com.apple.quarantine llvm16.0
	export PATH=$(pwd)/llvm16.0/bin:$PATH

.. _llvm-from-source:

Building LLVM from source
~~~~~~~~~~~~~~~~~~~~~~~~~

The LLVM project itself has a guide to `installing from source <http://www.llvm.org/docs/CMake.html>`_ which
you may need to consult. `Ninja <https://ninja-build.org>`_ is necessary for building LLVM from source. First of all, clone our LLVM repository:

.. code-block:: bash

	git clone --depth 1 --branch solana-rustc/15.0-2022-12-07 https://github.com/solana-labs/llvm-project
	cd llvm-project

Now run cmake to create the makefiles. Replace the *installdir* argument to ``CMAKE_INSTALL_PREFIX`` with a directory where you would like to have LLVM installed, and then run the build:

.. code-block:: bash

    cmake -G Ninja -DLLVM_ENABLE_ASSERTIONS=On '-DLLVM_ENABLE_PROJECTS=clang;lld'  \
        -DLLVM_ENABLE_TERMINFO=Off -DCMAKE_BUILD_TYPE=Release \
        -DLLVM_ENABLE_ZSTD=Off \
        -DCMAKE_INSTALL_PREFIX=installdir -B build llvm
    cmake --build build --target install

Once the build has succeeded, the *installdir*/bin has to be added to your path so the
Solang build can find the ``llvm-config`` from this build:

.. code-block:: bash

	export PATH=installdir/bin:$PATH

And on Windows, assuming *installdir* was ``C:\Users\User\solang-llvm``:

.. code-block:: batch

	set PATH=%PATH%;C:\Users\User\solang-llvm\bin

.. _build-from-source:

Step 2: Build Solang
____________________

Once you have the correct LLVM version in your path, ensure you have GNU make installed and simply run:

.. code-block:: bash

	git clone https://github.com/hyperledger/solang/
	cd solang
	cargo build --release

The executable will be in ``target/release/solang``.

.. _build-from-crates:

Alternative step 2: Build Solang from crates.io
_______________________________________________

The latest Solang release is  on `crates.io <https://crates.io/crates/solang>`_. Once you have the
correct LLVM version in your path, ensure you have GNU make installed and simply run:

.. code-block:: bash

	cargo install solang
