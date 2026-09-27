import { define as defineTabs }    from "@components/tabs";
import { define as defineModal }   from "@components/modal";
import { define as defineLabeled } from "@components/fieldset";
import { navigationUnit } from "@units/navigation";
import { queueUnit }      from "@units/queue";
import { reportUnit }     from "@units/report";
import { editorUnit }     from "@units/editor";
import { settingsUnit }   from "@units/settings";
import { shellUnit }      from "@units/shell";

defineTabs();
defineModal("ram-modal");
defineLabeled("ram-labeled");

window.addEventListener("DOMContentLoaded", main);

const units: (() => void | Promise<void>)[] = [
	settingsUnit,
	queueUnit,
	reportUnit,
	editorUnit,
	navigationUnit,
	shellUnit
];

async function main() {
	for (const unit of units) await unit();
}
