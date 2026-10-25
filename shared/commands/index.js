/**
 * The command surface: one registry, one dispatcher, every transport.
 *
 * `buildRegistry()` is the single place a command is added, and it is what makes "a click, a
 * `curl` and an agent run the same code" true by construction rather than by discipline —
 * there is nowhere else to put one (plan §3, Layer E).
 *
 * @module shared/commands
 */

import { Registry } from './registry.js';
import { dispatch, validateParams } from './dispatch.js';
import { projectCommands } from './commands/project.js';
import { pageCommands } from './commands/page.js';
import { panelCommands } from './commands/panel.js';
import { artworkCommands } from './commands/artwork.js';
import { letteringCommands } from './commands/lettering.js';
import { letteringToolCommands } from './commands/lettering-tools.js';
import { pageToolCommands } from './commands/page-tools.js';
import { panelToolCommands } from './commands/panel-tools.js';
import { storyCommands } from './commands/story.js';
import { generationCommands } from './commands/generation.js';
import { assetCommands } from './commands/asset.js';
import { characterCommands } from './commands/character.js';
import { agentCommands } from './commands/agent.js';
import { publishCommands } from './commands/publish.js';
import { editCommands } from './commands/edit.js';
import { exportCommands } from './commands/export.js';

/**
 * @returns {Registry}
 */
export function buildRegistry() {
  return new Registry().registerAll([
    ...projectCommands,
    ...pageCommands,
    ...panelCommands,
    ...artworkCommands,
    ...letteringCommands,
    ...letteringToolCommands,
    ...pageToolCommands,
    ...panelToolCommands,
    ...storyCommands,
    ...generationCommands,
    ...assetCommands,
    ...characterCommands,
    ...agentCommands,
    ...publishCommands,
    ...editCommands,
    ...exportCommands,
  ]);
}

export { Registry, dispatch, validateParams };
export { fail, ok } from './registry.js';
export { projectCommands, pageCommands, panelCommands, artworkCommands, letteringCommands, letteringToolCommands, pageToolCommands, panelToolCommands, storyCommands, generationCommands, assetCommands, characterCommands, agentCommands, publishCommands, editCommands, exportCommands };
