import type { ToolDefinition } from "./contracts.js";

export const repobrainToolDefinitions: ToolDefinition[] = [
  {
    name: "get_brief",
    description: "Compile a task-aware repo briefing pack under a token budget.",
    inputSchema: {
      type: "object",
      required: ["goal", "scope", "tokenBudget"],
      properties: {
        goal: { type: "string" },
        scope: { type: "string" },
        tokenBudget: { type: "integer", minimum: 1 },
      },
    },
  },
  {
    name: "explain_flow",
    description: "Explain a repository flow with the requested depth.",
    inputSchema: {
      type: "object",
      required: ["flowName", "depth"],
      properties: {
        flowName: { type: "string" },
        depth: { type: "string", enum: ["compact", "standard", "deep"] },
      },
    },
  },
  {
    name: "blast_radius",
    description: "Estimate the likely impact radius of changing a target.",
    inputSchema: {
      type: "object",
      required: ["target"],
      properties: {
        target: { type: "string" },
      },
    },
  },
  {
    name: "list_invariants",
    description: "Return invariants and do-not-break constraints for a scope.",
    inputSchema: {
      type: "object",
      required: ["scope"],
      properties: {
        scope: { type: "string" },
      },
    },
  },
  {
    name: "what_changed_semantically",
    description: "Summarize meaning-level repo changes for a ref.",
    inputSchema: {
      type: "object",
      required: ["ref"],
      properties: {
        ref: { type: "string" },
      },
    },
  },
];
