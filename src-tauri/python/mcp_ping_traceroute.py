import subprocess
import sys
from mcp.server.fastmcp import FastMCP

# Initialize FastMCP server
mcp = FastMCP("NetOps_Ping_Traceroute")

@mcp.tool()
def ping(host: str, count: int = 4) -> str:
    """
    Execute a ping command to check network connectivity to a host.

    Args:
        host: The IP address or hostname to ping.
        count: The number of ping packets to send (default: 4).
    """
    try:
        # Use list of arguments as recommended for subprocess.run
        # Determine platform to use the right count flag
        count_flag = "-n" if sys.platform.startswith("win") else "-c"
        result = subprocess.run(
            ["ping", count_flag, str(count), host],
            capture_output=True,
            text=True,
            check=True
        )
        return result.stdout
    except subprocess.CalledProcessError as e:
        return f"Ping failed for {host}:\n{e.stdout}\n{e.stderr}"
    except Exception as e:
        return f"Error executing ping: {str(e)}"

@mcp.tool()
def traceroute(host: str) -> str:
    """
    Execute a traceroute command to trace the network path to a host.

    Args:
        host: The IP address or hostname to trace.
    """
    try:
        # Determine platform to use the right command
        cmd = "tracert" if sys.platform.startswith("win") else "traceroute"
        result = subprocess.run(
            [cmd, host],
            capture_output=True,
            text=True,
            check=True
        )
        return result.stdout
    except subprocess.CalledProcessError as e:
        return f"Traceroute failed for {host}:\n{e.stdout}\n{e.stderr}"
    except Exception as e:
        return f"Error executing traceroute: {str(e)}"

if __name__ == "__main__":
    # Run the MCP server using standard input/output
    mcp.run()
