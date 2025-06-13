"""Shared pytest configuration and fixtures."""

import asyncio
from collections.abc import Generator
from unittest.mock import MagicMock

import pytest


@pytest.fixture(scope="session")
def event_loop() -> Generator[asyncio.AbstractEventLoop, None, None]:
    """Create an instance of the default event loop for the test session."""
    loop = asyncio.get_event_loop_policy().new_event_loop()
    yield loop
    loop.close()


# Prevent tests from accessing real keychain
@pytest.fixture(autouse=True)
def mock_keyring(monkeypatch: pytest.MonkeyPatch) -> MagicMock:
    """Mock keyring to prevent access to real system keychain during tests."""
    mock = MagicMock()
    mock.get_password.return_value = None
    mock.set_password.return_value = None
    mock.delete_password.return_value = None

    monkeypatch.setattr("keyring.get_password", mock.get_password)
    monkeypatch.setattr("keyring.set_password", mock.set_password)
    monkeypatch.setattr("keyring.delete_password", mock.delete_password)

    return mock
