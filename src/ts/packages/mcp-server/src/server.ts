import { spawn } from "node:child_process";

import { repobrainToolDefinitions } from "./tool-definitions.js";

export interface RepobrainManifest {
  name: string;
  version: string;
  tools: typeof repobrainToolDefinitions;
}

type ToolName = (typeof repobrainToolDefinitions)[number]["name"];

export interface ToolExecutionRequest {
  toolName: ToolName;
  args: Record<string, unknown>;
  repoRoot?: string;
  revision?: string;
  cliPath?: string;
}

export interface ToolExecutionResult {
  stdout: string;
  stderr: string;
  exitCode: number;
}

export function createRepobrainManifest(): RepobrainManifest {
  return {
    name: "repobrain-mcp-surface",
    version: "0.1.0",
    tools: repobrainToolDefinitions,
  };
}

export async function executeRepobrainTool(
  request: ToolExecutionRequest,
): Promise<ToolExecutionResult> {
  const program = request.cliPath ?? "repobrain";
  const commandArgs = buildCommandArgs(request);

  return new Promise<ToolExecutionResult>((resolve, reject) => {
    const child = spawn(program, commandArgs, {
      stdio: ["ignore", "pipe", "pipe"],
    });
    let stdout = "";
    let stderr = "";

    child.stdout?.on("data", (chunk: Buffer) => {
      stdout += chunk.toString("utf8");
    });
    child.stderr?.on("data", (chunk: Buffer) => {
      stderr += chunk.toString("utf8");
    });
    child.on("error", (error: Error) => {
      reject(error);
    });
    child.on("close", (exitCode) => {
      const normalizedExitCode = exitCode ?? -1;
      if (normalizedExitCode !== 0) {
        reject(
          new Error(
            `repobrain command failed (${normalizedExitCode}): ${program} ${commandArgs.join(" ")}\n${stderr.trim()}`,
          ),
        );
        return;
      }

      resolve({
        stdout,
        stderr,
        exitCode: normalizedExitCode,
      });
    });
  });
}

function buildCommandArgs(request: ToolExecutionRequest): string[] {
  const command = commandArgsForTool(request.toolName, request.args);
  if (request.repoRoot) {
    command.push("--repo-root", request.repoRoot);
  }
  if (request.revision) {
    command.push("--revision", request.revision);
  }

  return command;
}

function commandArgsForTool(toolName: ToolName, args: Record<string, unknown>): string[] {
  switch (toolName) {
    case "get_brief":
      return [
        "get-brief",
        "--goal",
        requiredStringArg(args, "goal"),
        "--scope",
        requiredStringArg(args, "scope"),
        "--token-budget",
        requiredIntegerArg(args, "tokenBudget"),
      ];
    case "explain_flow":
      return [
        "explain-flow",
        "--flow-name",
        requiredStringArg(args, "flowName"),
        "--depth",
        optionalDepthArg(args),
      ];
    case "blast_radius":
      return ["blast-radius", requiredStringArg(args, "target")];
    case "list_invariants":
      return ["list-invariants", "--scope", requiredStringArg(args, "scope")];
    case "what_changed_semantically":
      return ["what-changed-semantically", "--ref", requiredStringArg(args, "ref")];
    default:
      throw new Error(`Unsupported tool name: ${String(toolName)}`);
  }
}

function requiredStringArg(args: Record<string, unknown>, key: string): string {
  const value = args[key];
  if (typeof value !== "string" || value.length === 0) {
    throw new Error(`Missing required string argument: ${key}`);
  }

  return value;
}

function requiredIntegerArg(args: Record<string, unknown>, key: string): string {
  const value = args[key];
  if (typeof value !== "number" || !Number.isInteger(value) || value < 1) {
    throw new Error(`Missing required positive integer argument: ${key}`);
  }

  return String(value);
}

function optionalDepthArg(args: Record<string, unknown>): string {
  const value = args.depth;
  if (value === "compact" || value === "standard" || value === "deep") {
    return value;
  }

  return "standard";
}
