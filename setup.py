from setuptools import setup, find_packages

setup(
    name="physis-pro",
    version="0.1.0",
    description="Physis Pro MCP Gateway and Sidecar Manager",
    packages=find_packages(),
    install_requires=[
        "mcp",
        "httpx",
        "psutil",
    ],
    entry_points={
        "console_scripts": [
            "physis-gateway=gateway.physis_mcp_gateway:main",
        ],
    },
    python_requires=">=3.9",
)
