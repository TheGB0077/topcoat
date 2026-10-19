// @vitest-environment happy-dom
import { afterEach, expect, it, vi } from "vitest";

import {
	startTypedTransition,
	VT_TYPES_ATTRIBUTE,
	vtTypesOf,
} from "./view-transition";

afterEach(() => {
	vi.restoreAllMocks();
	delete (document as unknown as Record<string, unknown>).startViewTransition;
	document.body.innerHTML = "";
});

/** A startViewTransition double recording calls and running the updates. */
function stubViewTransition(throwOnOptions = false): {
	calls: unknown[];
} {
	const calls: unknown[] = [];
	(document as unknown as Record<string, unknown>).startViewTransition = (
		options: unknown,
	) => {
		calls.push(options);
		if (throwOnOptions && typeof options !== "function") {
			throw new TypeError("not callable");
		}
		const update =
			typeof options === "function"
				? options
				: (options as { update: () => void }).update;
		update();
		return {
			updateCallbackDone: Promise.resolve(),
			finished: Promise.resolve(),
		};
	};
	return { calls };
}

/** Renders `attrs` on a throwaway anchor and returns the parsed types. */
function parse(attrs: Record<string, string>): string[] | null {
	document.body.innerHTML = `<a id="t" ${Object.entries(attrs)
		.map(([name, value]) => `${name}="${value}"`)
		.join(" ")}></a>`;
	return vtTypesOf(document.getElementById("t") as Element);
}

it("reads the types an element declares", () => {
	// The attribute name is the contract apps write, so pin it literally
	// rather than through the constant.
	document.body.innerHTML = `<a data-topcoat-vt-types="expand"></a>`;
	expect(vtTypesOf(document.querySelector("a") as Element)).toEqual(["expand"]);

	expect(parse({ [VT_TYPES_ATTRIBUTE]: "expand fade" })).toEqual([
		"expand",
		"fade",
	]);
	// Surrounding whitespace is layout, not part of a type name.
	expect(parse({ [VT_TYPES_ATTRIBUTE]: "  expand \n fade " })).toEqual([
		"expand",
		"fade",
	]);
});

it("treats a missing or empty declaration as no transition", () => {
	expect(parse({})).toBeNull();
	expect(parse({ [VT_TYPES_ATTRIBUTE]: "" })).toBeNull();
	expect(parse({ [VT_TYPES_ATTRIBUTE]: "   " })).toBeNull();
});

it("applies the change directly where the API is missing", async () => {
	let ran = false;

	await startTypedTransition(["expand"], () => {
		ran = true;
	});

	expect(ran).toBe(true);
});

it("applies the change directly under reduced motion", async () => {
	const vt = stubViewTransition();
	vi.spyOn(window, "matchMedia").mockReturnValue({
		matches: true,
	} as MediaQueryList);
	let ran = false;

	await startTypedTransition(["expand"], () => {
		ran = true;
	});

	expect(ran).toBe(true);
	expect(vt.calls).toHaveLength(0);
});

it("wraps the change in a typed transition otherwise", async () => {
	const vt = stubViewTransition();
	let ran = false;

	await startTypedTransition(["expand"], () => {
		ran = true;
	});

	expect(vt.calls).toEqual([
		{ types: ["expand"], update: expect.any(Function) },
	]);
	expect(ran).toBe(true);
});

it("still transitions where the options form is unsupported", async () => {
	const vt = stubViewTransition(true);
	let ran = false;

	await startTypedTransition(["expand"], () => {
		ran = true;
	});

	// The typed call throws during overload resolution, before the DOM is
	// touched, so retrying plainly animates without the app's keyframes.
	expect(vt.calls).toHaveLength(2);
	expect(typeof vt.calls[1]).toBe("function");
	expect(ran).toBe(true);
});
