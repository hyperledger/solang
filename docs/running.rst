Using Solang on the command line
================================

The Solang compiler is run on the command line. The solidity source file
names are provided as command line arguments; the output is an optimized
WebAssembly or Solana SBF file which is ready for deployment on a chain, and an metadata
file (also known as the abi).

The following targets are supported right now:
`Solana <https://www.solana.com/>`_ and
`Polkadot <https://substrate.io/>`_ (via the ``contracts`` pallet runtime).

Solang supports auto-completion for multiple shells. Use ``solang shell-complete --help`` to
learn whether your favorite shell is supported. If so, evaluate the output of
``solang shell-complete <SHELL>`` in order to activate it. Example installation with ``bash``:

.. code-block:: bash

	  echo 'source <(solang shell-complete bash)' >> ~/.bashrc


Compiler Usage
______________

  solang compile [OPTIONS]... [SOLIDITY SOURCE FILE]...

This means that the command line is ``solang compile`` followed by any options described below,
followed by one or more solidity source filenames.

Options:

-v, \-\-verbose
  Make the output more verbose. The compiler tell you what contracts have been
  found in the source, and what files are generated. Without this option Solang
  will be silent if there are no errors or warnings.

\-\-target *target*
  This takes one argument, which can either be ``solana`` or ``polkadot``. The target
  must be specified.

\-\-address\-length *length-in-bytes*
  Change the default address length on Polkadot. By default, Substate uses an address type of 32 bytes. This option
  is ignored for any other target.

\-\-value\-length *length-in-bytes*
  Change the default value length on Polkadot. By default, Substate uses an value type of 16 bytes. This option
  is ignored for any other target.

-o, \-\-output *directory*
  Sets the directory where the output should be saved. This defaults to the current working directory if not set.

\-\-output\-meta *directory*
  Sets the directory where metadata should be saved. For Solana, the metadata is the Anchor IDL file,
  and, for Polkadot, the .contract file. If this option is not set, the directory specified by ``--output``
  is used, and if that is not set either, the current working directory is used.

\-\-contract *contract-name* [, *contract-name*]...
  Only compile the code for the specified contracts. If any those contracts cannot be found, produce an error.

-O *optimization level*
  This takes one argument, which can be ``0``, ``1``, ``2``, ``3``, ``s``,
  or ``z``. These correspond to standard GCC/Clang optimization levels.
  ``s`` optimizes for size and ``z`` aggressively optimizes for size.
  The default is ``2``. The old values ``none``, ``less``, ``default``,
  and ``aggressive`` are still accepted for backward compatibility.

\-\-importpath *directory*
  When resolving ``import`` directives, search this directory. By default ``import``
  will only search the current working directory. This option can be specified multiple times
  and the directories will be searched in the order specified.

\-\-importmap *map=directory*
  When resolving ``import`` directives, if the first part of the path matches *map*,
  search the directory provided for the file. This option can be specified multiple times
  with different values for map.

\-\-help, -h
  This displays a short description of all the options

\-\-standard-json
  This option causes Solang to emulate the behaviour of Solidity
  `standard json output <https://solidity.readthedocs.io/en/v0.5.13/using-the-compiler.html#output-description>`_. No output files are written, all the
  output will be in json on stdout.

\-\-emit *phase*
  This option is can be used for debugging Solang itself. This is used to
  output early phases of compilation.

  Phase:

  ast-dot
    Output Abstract Syntax Tree as a graphviz dot file. This can be viewed with xdot
    or any other tool that can visualize graphviz dot files.

  cfg
    Output control flow graph.

  llvm-ir
    Output llvm IR as text.

  llvm-bc
    Output llvm bitcode as binary file.

  asm
    Output assembly text file.

  object
    Output wasm object file; this is the contract before final linking.

\-\-no\-constant\-folding
   Disable the :ref:`constant-folding` codegen optimization

\-\-no\-strength\-reduce
   Disable the :ref:`strength-reduce` codegen optimization

\-\-no\-dead\-storage
   Disable the :ref:`dead-storage` optimization

\-\-no\-vector\-to\-slice
   Disable the :ref:`vector-to-slice` optimization

\-\-no\-cse
   Disable the :ref:`common-subexpression-elimination` optimization

\-\-no\-log\-runtime\-errors
   Disable the :ref:`no-log-runtime-errors` debugging feature

\-\-no\-prints
   Disable the :ref:`no-print` debugging feature

\-\-release
   Disable all debugging features for :ref:`release`

\-\-config-file
  Read compiler configurations from a ``.toml`` file. The minimal fields required in the configuration file are:
   
  .. code-block:: toml

    [package]
    input_files = ["flipper.sol"]  # Solidity files to compile

    [target]
    name = "solana"  # Target name


  Fields not explicitly present in the .toml acquire the compiler's default value.
  If any other argument is provided in the command line, for example, ``solang compile --config-file --target polkadot``, the argument will be overridden.
  The priority for the args is given as follows:
  1. Command line
  2. Configuration file
  3. Default values.
  The default name for the toml file is "solang.toml". If two configuration files exist in the same directory, priority will be given to the one passed explicitly to this argument.
  
\-\-wasm-opt
   wasm-opt passes for Wasm targets (0, 1, 2, 3, 4, s or z; see the wasm-opt help for more details).

\-\-contract-authors
   Specify authors for all contracts. If a `@author` tag is present, it will override this argument for the targeted contract.
   For specifying multiple authors, use this format: `--contract-authors author1,author2,..`

  .. note::

      This will only affect the metadata in case of substrate target.

\-\-version
   Specify contracts version. According to `semver <https://semver.org/>`_, a normal version number must take the form X.Y.Z where X, Y, and Z are non-negative integers, and must not contain leading zeroes.

.. warning::

    If multiple Solidity source files define the same contract name, you will get a single
    compiled contract file for this contract name. As a result, you will only have a single
    contract with the duplicate name without knowing from which Solidity file it originated.
    Solang will not give a warning about this problem.



Starting a new project
______________________________


  solang new \-\-target solana my_project

A solang project is a directory in which there are one or more solidity files and a ``solang.toml`` file where 
the compilation options are defined. Given these two components, a user can run ``solang compile`` in a similar fashion as ``cargo build``.

The ``solang new`` command creates a new solang project with an example `flipper <https://github.com/hyperledger-solang/solang/blob/main/examples/solana/flipper.sol>`_ contract,
and a default ``solang.toml`` configuration file.



Generating Documentation Usage
______________________________

Generate documentation for the given Solidity files as a single html page. This uses the
doccomment tags. The result is saved in ``soldoc.html``. See :ref:`tags` for
further information.

  solang doc [OPTIONS]... [SOLIDITY SOURCE FILE]...

This means that the command line is ``solang doc`` followed by any options described below,
followed by one or more solidity source filenames.

Options:

\-\-target *target*
  This takes one argument, which can either be ``solana`` or ``polkadot``. The target
  must be specified.

\-\-address\-length *length-in-bytes*
  Change the default address length on Polkadot. By default, Substate uses an address type of 32 bytes. This option
  is ignored for any other target.

\-\-value\-length *length-in-bytes*
  Change the default value length on Polkadot. By default, Substate uses an value type of 16 bytes. This option
  is ignored for any other target.

\-\-importpath *directory*
  When resolving ``import`` directives, search this directory. By default ``import``
  will only search the current working directory. This option can be specified multiple times
  and the directories will be searched in the order specified.

\-\-importmap *map=directory*
  When resolving ``import`` directives, if the first part of the path matches *map*,
  search the directory provided for the file. This option can be specified multiple times
  with different values for map.

\-\-help, -h
  This displays a short description of all the options

.. _idl_command:

Generate Solidity interface from IDL
____________________________________

This command converts Anchor IDL into Solidity import files, so they can be used to call
Anchor Programs from Solidity.

  solang idl [--output DIR] [IDLFILE]...

For each idl file provided, a Solidity file is written. See :ref:`call_anchor`
for an example of how to use this.

.. note::

  There is only supported on Solana.

Running Solang using a container
________________________________

First pull the last Solang container from
`solang containers <https://github.com/hyperledger-solang/solang/pkgs/container/solang>`_:

.. code-block:: bash

    docker image pull ghcr.io/hyperledger/solang

And if you are using podman:

.. code-block:: bash

    podman image pull ghcr.io/hyperledger/solang

Now you can run Solang like so:

.. code-block:: bash

	  docker run --rm -it ghcr.io/hyperledger/solang --version

Or podman:

.. code-block:: bash

	  podman container run --rm -it ghcr.io/hyperledger/solang --version

If you want to compile some Solidity files, the source files need to be
available inside the container. You can do this via the ``-v`` docker command line.
In this example ``/local/path`` should be replaced with the absolute path
to your solidity files:

.. code-block:: bash

	  docker run --rm -it -v /local/path:/sources ghcr.io/hyperledger/solang compile -o /sources /sources/flipper.sol

On Windows, you need to specify absolute paths:

.. code-block:: text

	 docker run --rm -it -v C:\Users\User:/sources ghcr.io/hyperledger/solang compile -o /sources /sources/flipper.sol
