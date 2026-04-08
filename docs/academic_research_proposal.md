# RepoBrain OS: A Deterministic Context Engine for AI Coding Assistants
**Research Project Proposal & Architectural Overview**

## 1. Executive Summary 

**The Vision:** 
RepoBrain OS is a continuous learning system that maps out a software codebase and automatically builds an evidence-backed, self-refreshing "brain" for AI agents. 

**The Core Problem:** 
Currently, AI coding assistants operate with a major handicap: they treat every question as if they are looking at the codebase for the very first time. Because large-scale software code is too massive for an AI to read completely on every request, AIs often guess how different files connect. This leads to confident but incorrect suggestions (hallucinations), broken code connections, and a massive waste of processing power. 

**The Research Goal (Why this matters):** 
This project aims to prove that we can dramatically improve how reliably an AI writes code by stripping away the guesswork. Instead of asking the AI to guess how a project holds together, RepoBrain OS feeds it mathematically certain, proven "facts" about the codebase. This determines whether feeding an AI structured architectural data is more effective than the industry standard of feeding it raw, unstructured text.

---

## 2. The Core Innovations

RepoBrain OS is built on three major technical pillars that separate it from typical search tools or standard AI plugins.

### Innovation 1: The Repository Cognition Engine (Fact-Based Understanding)
Instead of a simple text search, this engine builds a deep relationship map of the entire project. 
*   **Facts over Guesses:** It prioritizes absolute truths (e.g., "File A explicitly imports File B") over AI guesses.
*   **Proof tracking:** Every time the system claims a piece of knowledge about the project's structure, it attaches an "Evidence Receipt"—a proven trail back to the exact line of code that confirms it.
*   **Adapting to the AI:** The system actively changes how much assistance it provides depending on whether the user is running a very powerful AI or a smaller, cheaper AI. It provides more step-by-step guidance to weaker models.

### Innovation 2: Contract-Based Code Extraction 
Software projects often use multiple programming languages. A common failure in complex systems is that the blueprint for how data should look gets distorted between these languages. 
*   **Single Source of Truth:** RepoBrain OS enforces a strict central blueprint (using a standard called JSON Schema). The rules for Python, Rust, and TypeScript parts of the project are all automatically generated from this one master blueprint so they never drift out of sync.
*   **Tiered Understanding:** The system extracts information in safe stages. It will never claim to understand the deeper meaning of a piece of code if it only has access to a surface-level grammar check. It maintains an honest "capability limit" to prevent misleading the AI. 

### Innovation 3: Latency-First (Speed-First) Architecture
A tool is useless if a software engineer has to wait 30 seconds every time they ask a question.
*   **Zero "Hot Path" Blockers:** When a user asks a question, the system does not scan the whole codebase. It relies entirely on a continuously maintained background snapshot that is kept up-to-date quietly.
*   **Speed Tiers:** The system explicitly classifies tasks by urgency to prevent freezing the user's workflow.

---

## 3. Technology Stack & Key Metrics

To achieve a production-ready system capable of handling massive software systems, RepoBrain OS utilizes a highly specific and optimized technology stack.

### Technologies
*   **Rust (System Core):** Used for the background engine due to its safe memory management and extremely fast performance when mapping out code structures.
*   **Incremental Parsers (Tree-sitter):** A high-speed tool that reads code grammar in fractions of a second, only updating the exact lines of code a user just changed.
*   **SQLite FTS5:** An embedded, ultra-fast database technology used to rank and map text connections locally without needing the cloud.
*   **Python:** Used for evaluating the system's performance and researching new models. 
*   **TypeScript:** Used to integrate the engine into practical user interfaces and standard AI protocols.

### Perfected Key Metrics (Target Service Level Agreements)
The system's performance is strictly governed by mathematically enforced speed limits based on the user's need:
*   **Instant Class Queries:** Returns responses in under **150 milliseconds** (e.g., searching for a specific function name).
*   **Interactive Class Queries:** Returns responses in under **800 milliseconds** (e.g., summarizing how a short piece of code works).
*   **Deep Class Queries:** Returns full architecture investigations in under **2.5 seconds** (e.g., finding the exact "blast radius" or consequence of deleting a core security feature).

---

## 4. Academic Contribution & Research Worthiness

**Why this is a strong thesis/research project for academic review:**

1.  **Challenges Industry Consensus:** The current industry trend relies on throwing increasingly larger memory budgets (context windows) at reasoning models. RepoBrain investigates the counter-hypothesis: computationally cheap, deterministic preprocessing of code actually acts as a massive multiplier for AI reasoning capabilities.
2.  **Measurable Hypotheses:** The project allows for strictly defined experiments. We can measure exact variables: 
    *   *Does supplying exact "Evidence Receipts" reduce the hallucination rate of AI models?*
    *   *Do "speed-first" background snapshot queries fundamentally improve the human-in-the-loop developer experience compared to live-processing loops?*
3.  **Applies Distributed Systems Concepts to AI:** It successfully resolves standard distributed systems problems—staleness, cache invalidation, and data drift—and applies them specifically to the field of AI contextual grounding. 

RepoBrain OS is not simply another AI wrapper; it is a fundamental infrastructure layer that acts as the missing "reliable memory" for next-generation automated software engineering.
