"""Loxone MCP Server - Control Loxone Gen 1 systems via Model Context Protocol."""

from .server import mcp

__version__ = "0.1.0"
__all__ = ["mcp"]


def run() -> None:
    """Main entry point for loxone-mcp command with subcommands."""
    import argparse
    import sys

    parser = argparse.ArgumentParser(
        prog='loxone-mcp',
        description='Loxone MCP Server - Control Loxone Gen 1 systems via Model Context Protocol'
    )

    subparsers = parser.add_subparsers(dest='command', help='Available commands')

    # Setup command
    setup_parser = subparsers.add_parser(
        'setup', help='Configure Loxone credentials (with Infisical support)'
    )
    setup_parser.add_argument('--host', help='Miniserver IP address (e.g., 192.168.1.100)')
    setup_parser.add_argument('--username', help='Username for Miniserver')
    setup_parser.add_argument('--password', help='Password for Miniserver')
    setup_parser.add_argument('--api-key', help='SSE API key (optional)')
    setup_parser.add_argument(
        '--no-discovery', action='store_true', help='Disable automatic server discovery'
    )
    setup_parser.add_argument(
        '--discovery-timeout', type=float, default=5.0,
        help='Discovery timeout in seconds (default: 5.0)'
    )
    setup_parser.add_argument(
        '--non-interactive', action='store_true',
        help='Run in non-interactive mode (requires --host, --username, --password)'
    )

    # Other commands
    subparsers.add_parser('verify', help='Validate existing credentials')
    subparsers.add_parser('clear', help='Remove all stored credentials')
    subparsers.add_parser('migrate', help='Migrate keychain credentials to Infisical')
    subparsers.add_parser('server', help='Run MCP server (stdio mode)')
    subparsers.add_parser('sse', help='Run SSE server for web integrations')

    # Parse arguments
    args = parser.parse_args()

    if not args.command:
        parser.print_help()
        sys.exit(1)

    if args.command == "setup":
        from .credentials import get_credentials_manager

        manager = get_credentials_manager()

        # Pass CLI arguments to setup
        setup_args = {
            'host': args.host,
            'username': args.username,
            'password': args.password,
            'api_key': args.api_key,
            'enable_discovery': not args.no_discovery,
            'discovery_timeout': args.discovery_timeout,
            'interactive': not args.non_interactive
        }

        manager.setup(**setup_args)
    elif args.command == "verify":
        from .credentials import get_credentials_manager

        manager = get_credentials_manager()
        if manager.validate():
            print("✅ All credentials are configured")
        else:
            sys.exit(1)
    elif args.command == "clear":
        from .credentials import get_credentials_manager

        manager = get_credentials_manager()
        manager.clear_all()
    elif args.command == "migrate":
        from .credentials import get_credentials_manager

        manager = get_credentials_manager()
        if hasattr(manager, 'migrate_from_keychain'):
            manager.migrate_from_keychain()
        else:
            print("❌ Migration not available with current credential backend")
            sys.exit(1)
    elif args.command == "server":
        # Run stdio MCP server
        import asyncio

        asyncio.run(mcp.run_stdio_async())
    elif args.command == "sse":
        # Run SSE server
        from .sse_server import main

        main()
    else:
        print(f"Unknown command: {args.command}")
        print("Available commands: setup, verify, clear, migrate, server, sse")
        sys.exit(1)
