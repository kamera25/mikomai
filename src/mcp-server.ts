import { Server } from "@modelcontextprotocol/sdk/server/index.js";
import { StdioServerTransport } from "@modelcontextprotocol/sdk/server/stdio.js";
import {
  CallToolRequestSchema,
  ListToolsRequestSchema,
} from "@modelcontextprotocol/sdk/types.js";
import * as fs from "fs";
import * as path from "path";

// Create the MCP server
const server = new Server(
  {
    name: "csv-exporter-server",
    version: "1.0.0",
  },
  {
    capabilities: {
      tools: {},
    },
  }
);

// Register the tool to list tools
server.setRequestHandler(ListToolsRequestSchema, async () => {
  return {
    tools: [
      {
        name: "export_to_csv",
        description: "Exports an arbitrary string to a CSV file",
        inputSchema: {
          type: "object",
          properties: {
            content: {
              type: "string",
              description: "The content to write into the CSV file",
            },
            filename: {
              type: "string",
              description: "The filename of the CSV file (e.g. output.csv)",
            },
          },
          required: ["content", "filename"],
        },
      },
    ],
  };
});

// Implement the tool execution
server.setRequestHandler(CallToolRequestSchema, async (request) => {
  if (request.params.name === "export_to_csv") {
    const { content, filename } = request.params.arguments as {
      content: string;
      filename: string;
    };

    if (typeof content !== "string" || typeof filename !== "string") {
      throw new Error("Invalid arguments: content and filename must be strings");
    }

    try {
      // Ensure the filename ends with .csv
      const finalFilename = filename.endsWith(".csv")
        ? filename
        : `${filename}.csv`;

      // Resolve the full path, here writing to the current directory where the server is run
      const filePath = path.resolve(process.cwd(), finalFilename);

      // Write the file
      fs.writeFileSync(filePath, content, "utf8");

      return {
        content: [
          {
            type: "text",
            text: `Successfully wrote content to ${filePath}`,
          },
        ],
      };
    } catch (error: any) {
      return {
        content: [
          {
            type: "text",
            text: `Failed to write CSV file: ${error.message}`,
          },
        ],
        isError: true,
      };
    }
  }

  throw new Error(`Tool not found: ${request.params.name}`);
});

async function main() {
  const transport = new StdioServerTransport();
  await server.connect(transport);
  console.error("CSV Exporter MCP Server running on stdio");
}

main().catch((error) => {
  console.error("Fatal error in main():", error);
  process.exit(1);
});
