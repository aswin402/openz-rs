# Skill: Vague Issue Clarification
## Goal
Effectively handle vague or ambiguous user requests by systematically gathering necessary context before attempting to solve the problem.
## Description
When users provide minimal or unclear descriptions of issues (e.g., "browser and spreadsheet not connected"), use a structured approach to gather essential context before proceeding with troubleshooting.
## Rules/Guidelines
- **Don't Assume Context**: Never assume what the user means by vague terms like "browser", "spreadsheet", or "connected".
- **Systematic Questioning**: Ask targeted questions to clarify:
  - **Specific Tools**: Which exact browser/spreadsheet applications are involved?
  - **Integration Method**: What type of connection/API/extension is being used?
  - **Error Details**: What specific error messages or symptoms are observed?
  - **Project Context**: Is this related to a specific project or workspace?
  - **Environment Check**: Before asking questions, quickly scan the environment for:
    - Current directory structure and projects
    - Available tools and configurations (e.g., check `manage_mcp` for active servers)
    - Recent files or relevant configurations
- **Structured Response**: Present clarification questions in a clear, bulleted format for easy response
- **Avoid Premature Action**: Do not attempt complex troubleshooting until sufficient context is gathered
## Examples
### Example 1: Browser-Spreadsheet Connection Issue
**Vague Input**: "hey check why browser and spreadsheet not connected"
**Clarification Questions**:
- Which browser? (Chrome/Firefox/gsd_browser/other)
- Which spreadsheet? (Google Sheets/Excel/local file/OnlyOffice)
- What connection method? (API/extension/script/custom integration)
- What specific error or behavior are you seeing?
- Is this related to a specific project in your current directory?