import { describe, expect, it } from 'vitest';

import { HERD, Herd, agentForTool } from '../apps/desktop/sidecar/src/agents';

describe('the roster', () => {
  it('is six agents, all distinct, in a fixed display order', () => {
    expect(HERD).toHaveLength(6);
    expect(new Set(HERD.map((a) => a.id)).size).toBe(6);
    // Order matters: each has a permanent slot, so the row is learnable
    // spatially even though the six look identical (D18).
    expect(HERD.map((a) => a.id)).toEqual(['tuca', 'bia', 'zeca', 'nina', 'joca', 'duda']);
  });

  it('never assigns coral as an accent — coral belongs to "needs you"', () => {
    expect(HERD.map((a) => a.accent)).not.toContain('coral');
  });

  it('gives every agent a job description in plain words', () => {
    for (const agent of HERD) {
      expect(agent.label.length).toBeGreaterThan(0);
      expect(agent.job.length).toBeGreaterThan(0);
      expect(agent.job).not.toMatch(/[._]/); // no tool identifiers in the copy
    }
  });
});

describe('tool ownership', () => {
  it('sends each tool to the sensible specialist', () => {
    expect(agentForTool('fs.write')).toBe('nina');
    expect(agentForTool('fs.edit')).toBe('nina');
    expect(agentForTool('fs.delete')).toBe('zeca'); // the sceptic owns the irreversible
    expect(agentForTool('payment.send')).toBe('zeca');
    expect(agentForTool('web.search')).toBe('bia');
    expect(agentForTool('fs.read')).toBe('bia');
    expect(agentForTool('exec')).toBe('joca');
    expect(agentForTool('mail.send')).toBe('tuca'); // anything leaving the machine
  });

  it('fails safe to Duda for a tool nobody owns', () => {
    expect(agentForTool('something.brand.new')).toBe('duda');
  });
});

describe('herd state', () => {
  it('starts attentive: all six listening, none busy', () => {
    const snapshot = new Herd().snapshot();
    expect(snapshot).toHaveLength(6);
    expect(snapshot.every((a) => a.state === 'listening')).toBe(true);
  });

  it('puts the owner of the tool to work', () => {
    const herd = new Herd();
    expect(herd.beginWork('fs.write')).toBe('nina');
    expect(herd.snapshot().find((a) => a.id === 'nina')?.state).toBe('working');
  });

  it('never lets more than two agents look busy at once', () => {
    const herd = new Herd();
    herd.beginWork('fs.write'); // nina
    herd.beginWork('web.search'); // bia
    herd.beginWork('exec'); // joca — a third, so one must stand down
    const busy = herd.snapshot().filter((a) => a.state === 'working');
    expect(busy.length).toBeLessThanOrEqual(2);
  });

  it('never lets more than one agent need you at a time', () => {
    const herd = new Herd();
    herd.needsYou('nina');
    herd.needsYou('zeca');
    const needing = herd.snapshot().filter((a) => a.state === 'needs-you');
    expect(needing).toHaveLength(1);
    expect(needing[0]?.id).toBe('zeca');
  });

  it('does not let a new task silently overwrite a raised sign', () => {
    const herd = new Herd();
    herd.needsYou('nina');
    herd.beginWork('web.search'); // Bia starts working
    expect(herd.snapshot().find((a) => a.id === 'nina')?.state).toBe('needs-you');
  });

  it('lowers the sign when released', () => {
    const herd = new Herd();
    herd.needsYou('nina');
    herd.release('nina');
    expect(herd.snapshot().find((a) => a.id === 'nina')?.state).toBe('listening');
  });

  it('reports how long since each agent changed state', () => {
    let now = 1_000;
    const herd = new Herd(() => now);
    herd.beginWork('fs.write');
    now += 750;
    const nina = herd.snapshot().find((a) => a.id === 'nina');
    expect(nina?.sinceMs).toBe(750);
  });

  it('always returns all six, even when everything is idle', () => {
    // A list that omits idle agents cannot show that the herd is quiet,
    // and quiet is information.
    expect(new Herd().snapshot()).toHaveLength(6);
  });
});
