/**
 * The representative test book, built from commands rather than by hand.
 *
 * A three-panel page is a unit test. A book with two chapters, recurring characters, irregular
 * panels, long dialogue, different page sizes and a print export is what actually exercises
 * the model — and building it through the command surface means it cannot drift from what a
 * user or an agent can produce.
 *
 * @module test/book
 */

import { must } from './helpers.js';

/**
 * @param {any} session
 * @param {{ pages?: number, withData?: boolean }} [options]
 * @returns {Promise<{ chapters: string[], scenes: string[], pages: string[], panels: string[], characters: string[] }>}
 */
export async function buildTestBook(session, options = {}) {
  const withData = options.withData !== false;
  const extraPages = options.pages ?? 0;
  must(await session.run('comic.project.create', { title: 'The Long Night', pageWidth: 1275, pageHeight: 1950 }));

  const chapter1 = must(await session.run('comic.chapter.add', { title: 'Chapter 1: Rooftops' }));
  const chapter2 = must(await session.run('comic.chapter.add', { title: 'Chapter 2: Alleys' }));
  const rooftop = must(await session.run('comic.scene.add', { chapterId: chapter1.chapterId, name: 'Rooftop' }));
  const chase = must(await session.run('comic.scene.add', { chapterId: chapter1.chapterId, name: 'The chase' }));
  const alley = must(await session.run('comic.scene.add', { chapterId: chapter2.chapterId, name: 'Alley' }));

  const maya = must(await session.run('comic.character.add', { name: 'Maya' }));
  const jonas = must(await session.run('comic.character.add', { name: 'Jonas' }));
  must(await session.run('comic.character.update', {
    characterId: maya.characterId,
    canonicalDescription: 'Short black hair, red windbreaker, a scar over the left brow',
    continuityRules: ['the scar is on the left brow in every panel'],
  }));

  const pageIds = [];
  const panelIds = [];

  /** @param {string} sceneId @param {string} name @param {string|undefined} template @param {{width?: number, height?: number}} [size] */
  async function addPage(sceneId, name, template, size = {}) {
    const created = must(await session.run('comic.page.add', {
      name, sceneId, ...(template ? { template } : {}), ...size,
    }));
    pageIds.push(created.pageId);
    const ids = session.bundle.project.pages[created.pageId].panelIds;
    panelIds.push(...ids);
    return { pageId: created.pageId, panels: ids };
  }

  const p1 = await addPage(rooftop.sceneId, 'Rooftop, wide', 'three-up');
  const p2 = await addPage(rooftop.sceneId, 'The shadow', 'two-by-two');
  const p3 = await addPage(chase.sceneId, 'The chase', 'five-grid', { width: 1275, height: 1950 });
  const p4 = await addPage(alley.sceneId, 'The alley', 'single', { width: 900, height: 1400 });
  const p5 = await addPage(alley.sceneId, 'Print spread', 'two-up', { width: 2550, height: 1950 });

  // Irregular panels: a full-width establishing shot with two smaller reaction panels.
  const [a, b, c, d] = p2.panels;
  must(await session.run('comic.panel.setFrame', { panelId: a, x: 40, y: 40, width: 1195, height: 700 }));
  must(await session.run('comic.panel.setFrame', { panelId: b, x: 40, y: 780, width: 580, height: 1120 }));
  must(await session.run('comic.panel.setFrame', { panelId: c, x: 655, y: 780, width: 580, height: 540 }));
  must(await session.run('comic.panel.setFrame', { panelId: d, x: 655, y: 1360, width: 580, height: 540 }));

  const allPanels = [...p1.panels, ...p2.panels, ...p3.panels, ...p4.panels, ...p5.panels];
  for (let i = 0; i < allPanels.length; i++) {
    must(await session.run('comic.artwork.place', {
      panelId: allPanels[i],
      name: `Art ${i + 1}`,
      contentHash: `book-art-${i + 1}`,
      ...(withData ? { data: 'data:image/svg+xml;base64,PHN2ZyB4bWxucz0iaHR0cDovL3d3dy53My5vcmcvMjAwMC9zdmciLz4=' } : {}),
      provider: 'fixture',
      model: 'vector',
      request: `panel ${i + 1}`,
    }));
    must(await session.run('comic.artwork.clip', { panelId: allPanels[i], inset: 6 }));
  }

  must(await session.run('comic.lettering.add', {
    panelId: p1.panels[0], kind: 'balloon', variant: 'speech', characterId: maya.characterId,
    text: 'THE CITY IS QUIET TONIGHT, AND NOTHING IN IT KNOWS WHAT IS COMING.',
    overflowPolicy: 'shrink', x: 80, y: 120, width: 380, height: 180,
  }));
  must(await session.run('comic.lettering.add', {
    panelId: p1.panels[1], kind: 'caption',
    text: 'TOO QUIET.',
    overflowPolicy: 'warn', x: 460, y: 90, width: 360, height: 60,
  }));
  must(await session.run('comic.lettering.add', {
    panelId: p1.panels[2], kind: 'balloon', variant: 'thought',
    text: 'Who is there?',
    overflowPolicy: 'shrink', x: 820, y: 200, width: 360, height: 120,
  }));
  must(await session.run('comic.lettering.add', {
    panelId: p2.panels[1], kind: 'balloon', variant: 'whisper', characterId: jonas.characterId,
    text: 'Do not move.',
    overflowPolicy: 'shrink', x: 90, y: 900, width: 460, height: 130,
  }));
  must(await session.run('comic.lettering.add', {
    panelId: p3.panels[0], kind: 'sound-effect',
    text: 'KRAKOOM!',
    overflowPolicy: 'shrink', x: 140, y: 140, width: 500, height: 180,
  }));
  must(await session.run('comic.lettering.add', {
    panelId: p4.panels[0], kind: 'balloon', variant: 'shout', characterId: maya.characterId,
    text: 'RUN!',
    overflowPolicy: 'shrink', x: 120, y: 900, width: 400, height: 140,
  }));

  for (let i = 0; i < extraPages; i++) {
    const extra = await addPage(alley.sceneId, `Extra page ${i + 1}`, 'three-by-two');
    for (const panelId of extra.panels) {
      must(await session.run('comic.artwork.place', {
        panelId, name: `Extra art ${i}-${panelId}`, contentHash: `extra-${i}-${panelId}`,
        ...(withData ? { data: 'data:image/svg+xml;base64,PHN2Zy8+' } : {}),
      }));
    }
  }

  return {
    chapters: [chapter1.chapterId, chapter2.chapterId],
    scenes: [rooftop.sceneId, chase.sceneId, alley.sceneId],
    pages: pageIds,
    panels: panelIds,
    characters: [maya.characterId, jonas.characterId],
  };
}
