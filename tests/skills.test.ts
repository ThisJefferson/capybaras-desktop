import { describe, expect, it } from 'vitest';

import {
  SKILLS,
  headlineFor,
  moreCautious,
  ownerFor,
  skillFor,
  validateRegistry,
} from '../src/skills/registry';

describe('the registry as declared', () => {
  it('is valid', () => {
    expect(validateRegistry(SKILLS)).toEqual([]);
  });

  it('declares every tool the gate knows how to describe', () => {
    // If a tool is added to the classifier but not here, the card loses its
    // plain-language headline and starts showing a tool identifier.
    expect(SKILLS.length).toBeGreaterThanOrEqual(26);
    for (const tool of ['fs.read', 'fs.write', 'fs.delete', 'db.delete', 'exec', 'payment.send']) {
      expect(skillFor(tool), `${tool} should be declared`).toBeDefined();
    }
  });

  it('has no duplicate tools', () => {
    expect(new Set(SKILLS.map((s) => s.tool)).size).toBe(SKILLS.length);
  });

  it('never lets a headline leak a dotted tool identifier', () => {
    // Only DOTTED ids are identifiers. A single-word tool id like `spend` may
    // legitimately be an English word in its own headline ("Spend money").
    // An earlier version of this test flagged that as a leak -- the test was
    // too crude, not the registry.
    for (const skill of SKILLS.filter((s) => s.tool.includes('.'))) {
      expect(skill.headline.one.toLowerCase()).not.toContain(skill.tool.toLowerCase());
    }
  });

  it('gives every skill an owner that is a real capybara', () => {
    for (const skill of SKILLS) {
      expect(['tuca', 'bia', 'zeca', 'nina', 'joca', 'duda']).toContain(skill.owner);
    }
  });

  it('never lets something that leaves the machine floor at silent', () => {
    for (const skill of SKILLS.filter((s) => s.leavesMachine)) {
      expect(skill.floor).not.toBe('silent');
    }
  });

  it('does not contain the exfiltration combination without saying so', () => {
    // readsUntrusted + leavesMachine is the dangerous pair. None should have
    // both today; if one appears, validation flags it for deliberate review.
    const both = SKILLS.filter((s) => s.readsUntrusted && s.leavesMachine);
    expect(both).toEqual([]);
  });
});

describe('validation can actually refuse — every way a skill could be added carelessly', () => {
  const good = SKILLS.find((s) => s.tool === 'fs.read')!;
  const check = (patch: Record<string, unknown>) =>
    validateRegistry([{ ...good, ...patch } as never]);

  it('refuses a missing headline', () => expect(check({ headline: undefined })).not.toEqual([]));
  it('refuses a headline that names the tool', () =>
    expect(check({ headline: { one: 'Read a fs.read file' } })).not.toEqual([]));
  it('refuses a many-headline with no {n}', () =>
    expect(check({ headline: { one: 'Read a file', many: 'Read files' } })).not.toEqual([]));
  it('refuses a missing owner', () => expect(check({ owner: undefined })).not.toEqual([]));
  it('refuses an unknown owner', () => expect(check({ owner: 'nobody' })).not.toEqual([]));
  it('refuses an undeclared safety field', () =>
    expect(check({ leavesMachine: undefined })).not.toEqual([]));
  it('refuses a non-boolean safety field', () => expect(check({ reversible: 'yes' })).not.toEqual([]));
  it('refuses a missing floor', () => expect(check({ floor: undefined })).not.toEqual([]));
  it('refuses an unknown floor', () => expect(check({ floor: 'whatever' })).not.toEqual([]));
  it('refuses something that leaves the machine but floors at silent', () =>
    expect(check({ leavesMachine: true, floor: 'silent' })).not.toEqual([]));
  it('refuses the exfiltration combination without review', () =>
    expect(check({ readsUntrusted: true, leavesMachine: true })).not.toEqual([]));
  it('refuses a duplicate tool', () => expect(validateRegistry([good, good])).not.toEqual([]));
  it('refuses a missing tool name', () =>
    expect(validateRegistry([{ ...good, tool: undefined } as never])).not.toEqual([]));
});

describe('helpers', () => {
  it('formats singular and plural headlines', () => {
    expect(headlineFor('fs.read', 1)).toBe('Read a file');
    expect(headlineFor('fs.read', 4)).toBe('Read 4 files');
  });

  it('uses the singular when a plural form is not declared', () => {
    expect(headlineFor('exec', 9)).toBe('Run a command');
  });

  it('names an undeclared tool rather than inventing a sentence for it', () => {
    // An undeclared tool should be visible, not disguised behind plausible copy.
    expect(headlineFor('mystery.tool')).toContain('mystery.tool');
  });

  it('resolves owners', () => {
    expect(ownerFor('fs.write')).toBe('nina');
    expect(ownerFor('db.delete')).toBe('zeca');
    expect(ownerFor('mystery.tool')).toBeUndefined();
  });

  it('escalates upward only', () => {
    expect(moreCautious('silent', 'hard_gate')).toBe('hard_gate');
    expect(moreCautious('confirm', 'notify')).toBe('confirm');
    expect(moreCautious('notify', 'notify')).toBe('notify');
  });
});

describe('the PDF family — the first real suite', () => {
  const pdf = SKILLS.filter((s) => s.tool.startsWith('pdf.'));

  it('is declared', () => {
    expect(pdf.length).toBeGreaterThanOrEqual(13);
  });

  it('treats a PDF as untrusted input, because someone else wrote it', () => {
    for (const tool of ['pdf.read', 'pdf.extract-text', 'pdf.merge', 'pdf.convert']) {
      expect(skillFor(tool)?.readsUntrusted, `${tool} reads a document it did not write`).toBe(true);
    }
  });

  it('floors pure reading at silent', () => {
    for (const tool of ['pdf.read', 'pdf.extract-text', 'pdf.info', 'pdf.page-count']) {
      expect(skillFor(tool)?.floor).toBe('silent');
    }
  });

  it('floors creating at notify, because the originals are untouched', () => {
    for (const tool of ['pdf.create', 'pdf.merge', 'pdf.split', 'pdf.convert']) {
      expect(skillFor(tool)?.floor).toBe('notify');
    }
  });

  it('floors mutating at confirm', () => {
    for (const tool of ['pdf.overwrite', 'pdf.delete-pages', 'pdf.fill-form', 'pdf.sign', 'pdf.redact']) {
      expect(skillFor(tool)?.floor).toBe('confirm');
    }
  });

  it('floors unlocking and attaching at hard gate', () => {
    expect(skillFor('pdf.decrypt')?.floor).toBe('hard_gate');
    expect(skillFor('pdf.attach-file')?.floor).toBe('hard_gate');
  });

  it('says redaction can be silently wrong, and demands a post-condition', () => {
    const redact = skillFor('pdf.redact');
    expect(redact?.silentlyWrong).toBe(true);
    expect(redact?.postCondition).toBeTruthy();
    // The post-condition must be about VERIFYING the result, not about doing it.
    // A black rectangle over text leaves the text intact underneath, so "done"
    // is not the claim -- "gone" is.
    expect(redact?.postCondition?.toLowerCase()).toContain('verif');
  });
});

describe('skills whose failure is invisible', () => {
  const good = SKILLS.find((s) => s.tool === 'fs.read')!;

  it('are exactly the ones that need a post-condition, and all of them have one', () => {
    const flagged = SKILLS.filter((s) => s.silentlyWrong);
    expect(flagged.length).toBeGreaterThan(0);
    for (const skill of flagged) {
      expect(skill.postCondition, `${skill.tool} must state a postCondition`).toBeTruthy();
    }
  });

  it('refuses silent wrongness with no post-condition', () => {
    const problems = validateRegistry([{ ...good, silentlyWrong: true } as never]);
    expect(problems.join(' ')).toMatch(/postCondition/i);
  });

  it('refuses a post-condition on a skill that cannot be silently wrong', () => {
    const problems = validateRegistry([
      { ...good, silentlyWrong: false, postCondition: 'checked for no reason' } as never,
    ]);
    expect(problems.join(' ')).toMatch(/postCondition/i);
  });

  it('refuses an undeclared silentlyWrong', () => {
    expect(validateRegistry([{ ...good, silentlyWrong: undefined } as never])).not.toEqual([]);
  });
});
