# MCP Cloudflare Demo Server

A comprehensive MCP server running on Cloudflare Workers.

## Available Tools

### Calculator
- `add` - Add two numbers
- `subtract` - Subtract b from a
- `multiply` - Multiply two numbers
- `divide` - Divide a by b
- `sqrt` - Square root

### Text Utilities
- `uppercase` - Convert to uppercase
- `lowercase` - Convert to lowercase
- `reverse` - Reverse characters
- `word_count` - Count words
- `echo` - Echo with metadata

## Available Resources

### Static Resources
- `config://app` - Application configuration
- `info://server` - Server runtime information
- `docs://readme` - This documentation

### Resource Templates
- `user://{id}` - User profile by ID
- `doc://{id}` - Document by ID

## Available Prompts
- `code-review` - Review code for issues (with auto-completion!)
- `summarize` - Summarize text content
- `translate` - Translate to another language

## Usage

Initialize the connection first, then use tools, resources, or prompts as needed.
