from .core import *  # noqa: F401,F403


def validate_root_metadata_and_package_state(
    view: GitView, package: str, checks: Checks
) -> None:
    for root in CONTROL_ROOTS:
        files = view.list_files(root)
        root_metadata = {
            f"{root}/{name}" for name in ROOT_METADATA_NAMES
        }
        metadata_found = [path for path in files if path in root_metadata]
        if metadata_found:
            checks.pass_(
                f"root-metadata:{root}",
                f"root metadata present: {','.join(metadata_found)}",
            )
        else:
            checks.fail(
                f"root-metadata:{root}",
                "CONTROL_SCHEMA_MISMATCH",
                f"control root lacks exact root metadata: {root}",
            )
        nested_metadata = [
            path
            for path in files
            if path not in root_metadata
            and PurePosixPath(path).name in ROOT_METADATA_NAMES
        ]
        if nested_metadata:
            checks.fail(
                f"root-nested-metadata:{root}",
                "CURRENT_PACKAGE_CONTROL_RECORD_EXISTS",
                f"nested metadata filename is a record, not an exemption: {nested_metadata[0]}",
            )
        else:
            checks.pass_(
                f"root-nested-metadata:{root}",
                "no nested metadata filename bypass",
            )

    conflicts: list[str] = []
    for root in CURRENT_PACKAGE_RECORD_ROOTS:
        prefix = f"{root}/{package}"
        for path in view.list_files(prefix):
            conflicts.append(path)
    if conflicts:
        checks.fail(
            "current-package-records",
            "CURRENT_PACKAGE_CONTROL_RECORD_EXISTS",
            f"current-package control record already exists: {sorted(conflicts)[0]}",
        )
    else:
        checks.pass_(
            "current-package-records",
            "no current-package context/ticket/lease/submission/review/handoff record",
        )

    wave_records = [
        path
        for path in view.list_files("swarm/wave-receipts")
        if path
        not in {
            "swarm/wave-receipts/README.md",
            "swarm/wave-receipts/.gitkeep",
        }
    ]
    if wave_records:
        checks.fail(
            "w0-receipt",
            "W0_ALREADY_ACCEPTED",
            f"wave receipt already exists: {wave_records[0]}",
        )
    else:
        checks.pass_("w0-receipt", "no accepted wave receipt exists")


def validate_workflows(view: GitView, checks: Checks) -> None:
    files = [
        path
        for path in view.list_files(".github/workflows")
        if path.endswith((".yml", ".yaml"))
    ]
    violations: list[str] = []
    for path in files:
        try:
            text, _ = view.read_text(path)
        except PlannerFailure:
            violations.append(path)
            continue
        valid = (
            re.search(r"^\s{2}workflow_dispatch:\s*$", text, re.MULTILINE)
            is not None
            and FORBIDDEN_WORKFLOW_TRIGGER_RE.search(text) is None
            and re.search(r"^\s{2}contents:\s*read\s*$", text, re.MULTILINE)
            is not None
            and re.search(r"^\s{2}contents:\s*write\s*$", text, re.MULTILINE)
            is None
            and "persist-credentials: false" in text
        )
        if not valid:
            violations.append(path)
    if files and not violations:
        checks.pass_(
            "workflow-policy",
            f"{len(files)} workflows are manual/read-only/credential-free",
        )
    else:
        checks.fail(
            "workflow-policy",
            "WORKFLOW_POLICY_VIOLATION",
            f"workflow policy violation: {(violations or ['none found'])[0]}",
        )


def signed_payload_digest(raw: bytes) -> str | None:
    marker = b"\n[signature]\n"
    offset = raw.find(marker)
    if offset <= 0 or raw.find(marker, offset + 1) != -1:
        return None
    return exact_sha256(raw[: offset + 1])


def superseded_handoff_paths(view: GitView) -> set[str]:
    result: set[str] = set()
    for path in view.list_files("swarm/supersessions"):
        if path in {
            "swarm/supersessions/README.md",
            "swarm/supersessions/.gitkeep",
        }:
            continue
        try:
            record, _ = view.load_toml(path)
        except PlannerFailure:
            continue
        old = record.get("old_record")
        if not isinstance(old, dict):
            continue
        ref = old.get("ref")
        if isinstance(ref, dict) and isinstance(ref.get("path"), str):
            result.add(ref["path"])
    return result


def validate_handoffs(
    view: GitView,
    pair: DraftPair,
    paths: Sequence[str],
    checks: Checks,
) -> list[dict[str, Any]]:
    expected_from_slots = sorted(
        slot.split("::", 1)[0] for slot in pair.handoff_slots
    )
    dependencies = (
        pair.ticket.get("dependencies")
        if isinstance(pair.ticket.get("dependencies"), dict)
        else {}
    )
    try:
        expected_from_ticket = sorted(
            as_string_array(
                dependencies.get("required_handoff_packages"),
                "required_handoff_packages",
            )
        )
    except PlannerFailure:
        expected_from_ticket = []
    if expected_from_slots != expected_from_ticket:
        checks.fail(
            "handoff-topology",
            "DRAFT_PAIR_MISMATCH",
            "ticket dependencies and context handoff slots disagree",
        )
    else:
        checks.pass_(
            "handoff-topology",
            "ticket dependencies and context handoff slots agree",
        )

    supplied: dict[
        str, tuple[str, dict[str, Any], bytes, GitEntry]
    ] = {}
    invalid_or_duplicate = False
    for index, path in enumerate(paths):
        if not safe_path(path) or not under(path, "swarm/handoffs"):
            checks.fail(
                f"handoff-input-{index:02d}",
                "HANDOFF_RECORD_INVALID",
                f"unsafe handoff path: {path}",
            )
            invalid_or_duplicate = True
            continue
        try:
            raw, entry = view.read_bytes(path)
            record = tomllib.loads(raw.decode("utf-8", "strict"))
        except (
            PlannerFailure,
            UnicodeDecodeError,
            tomllib.TOMLDecodeError,
        ) as exc:
            detail = exc.message if isinstance(exc, PlannerFailure) else str(exc)
            checks.fail(
                f"handoff-input-{index:02d}",
                "HANDOFF_RECORD_INVALID",
                detail,
            )
            invalid_or_duplicate = True
            continue
        identity = (
            record.get("identity")
            if isinstance(record.get("identity"), dict)
            else {}
        )
        package = identity.get("package")
        handoff_id = identity.get("handoff_id")
        accepted = (
            record.get("accepted_code")
            if isinstance(record.get("accepted_code"), dict)
            else {}
        )
        public = (
            record.get("public_surface")
            if isinstance(record.get("public_surface"), dict)
            else {}
        )
        signature = (
            record.get("signature")
            if isinstance(record.get("signature"), dict)
            else {}
        )
        final_commit = accepted.get("final_commit")
        valid = (
            isinstance(package, str)
            and PACKAGE_RE.fullmatch(package) is not None
            and isinstance(handoff_id, str)
            and OPAQUE_ID_RE.fullmatch(handoff_id) is not None
            and path == f"swarm/handoffs/{package}/{handoff_id}.toml"
            and record.get("schema_version") == 1
            and record.get("record_kind") == "package_handoff_v1"
            and record.get("status") == "ACCEPTED"
            and identity.get("stage") == "W0"
            and isinstance(final_commit, str)
            and view.commit_exists(final_commit)
            and isinstance(public.get("api_schema_digest"), str)
            and SHA256_RE.fullmatch(public["api_schema_digest"]) is not None
            and isinstance(public.get("error_reason_digest"), str)
            and SHA256_RE.fullmatch(public["error_reason_digest"]) is not None
            and isinstance(signature.get("record_sha256"), str)
            and signature.get("record_sha256") == signed_payload_digest(raw)
        )
        if not valid or package in supplied:
            checks.fail(
                f"handoff-input-{index:02d}",
                "HANDOFF_RECORD_INVALID",
                f"handoff record failed canonical identity/readback checks: {path}",
            )
            invalid_or_duplicate = True
            continue
        supplied[package] = (path, record, raw, entry)
        checks.pass_(
            f"handoff-input-{index:02d}",
            f"canonical accepted handoff: {path}",
        )

    expected = expected_from_slots
    if sorted(supplied) != expected or invalid_or_duplicate:
        if set(expected) - set(supplied):
            checks.fail(
                "handoff-set-missing",
                "HANDOFF_SLOT_UNSATISFIED",
                "required accepted handoff is missing",
            )
        if (
            set(supplied) - set(expected)
            or invalid_or_duplicate
            or len(paths) != len(supplied)
        ):
            checks.fail(
                "handoff-set-extra",
                "HANDOFF_SET_UNEXPECTED",
                "unexpected, invalid or duplicate handoff supplied",
            )
    else:
        checks.pass_(
            "handoff-set",
            "accepted handoff package set exactly matches draft slots",
        )

    superseded = superseded_handoff_paths(view)
    result: list[dict[str, Any]] = []
    for package in sorted(supplied):
        path, record, raw, entry = supplied[package]
        if path in superseded:
            checks.fail(
                f"handoff-current-{package}",
                "HANDOFF_RECORD_SUPERSEDED",
                f"handoff is superseded: {path}",
            )
            continue
        identity = record["identity"]
        accepted = record["accepted_code"]
        public = record["public_surface"]
        checks.pass_(
            f"handoff-current-{package}", "handoff is not superseded"
        )
        result.append(
            {
                "package": package,
                "path": path,
                "handoff_id": identity["handoff_id"],
                "git_blob_id": view.blob_identity(entry),
                "exact_record_file_sha256": exact_sha256(raw),
                "accepted_commit": accepted["final_commit"],
                "api_schema_digest": public["api_schema_digest"],
                "error_reason_digest": public["error_reason_digest"],
            }
        )
    return result


def selection_state(
    base: str | None,
    writer: str | None,
    reviewer: str | None,
    checks: Checks,
) -> str:
    count = sum(value is not None for value in (base, writer, reviewer))
    if count == 0:
        checks.pass_("selection", "no issuance identity selected")
        return "NONE"
    if count != 3:
        checks.fail(
            "selection",
            "PARTIAL_ISSUANCE_SELECTION",
            "base commit, writer and reviewer must be supplied together",
        )
        return "PARTIAL"
    assert writer is not None and reviewer is not None
    if (
        ACTOR_RE.fullmatch(writer) is None
        or ACTOR_RE.fullmatch(reviewer) is None
    ):
        checks.fail(
            "actor-identities",
            "ACTOR_IDENTITY_INVALID",
            "writer or reviewer ActorIdentity is invalid",
        )
    else:
        checks.pass_(
            "actor-identities",
            "writer and reviewer use closed ActorIdentity grammar",
        )
    if writer == reviewer:
        checks.fail(
            "actor-independence",
            "WRITER_REVIEWER_CONFLICT",
            "writer and reviewer are identical",
        )
    else:
        checks.pass_(
            "actor-independence", "writer and reviewer identities differ"
        )
    return "COMPLETE"


def choose_decision(state: str, reasons: Iterable[str]) -> str:
    reason_set = set(reasons)
    if reason_set & INVALID_REASONS:
        return DECISION_INVALID
    if reason_set & CONFLICT_REASONS:
        return DECISION_CONFLICT
    if reason_set & PREREQUISITE_REASONS:
        return DECISION_PREREQUISITE
    if state == "NONE":
        return DECISION_MISSING
    if state != "COMPLETE":
        return DECISION_CONFLICT
    return DECISION_READY


def validate_output(
    root: Path, output: str, checks: Checks
) -> Path | None:
    if output == "-":
        checks.pass_("output-path", "stdout selected")
        return None
    relative = output.replace("\\", "/")
    valid = (
        safe_path(relative)
        and under(relative, PLAN_ARTIFACT_ROOT)
        and relative.endswith(".json")
        and relative != f"{PLAN_ARTIFACT_ROOT}/.json"
    )
    if not valid:
        checks.fail(
            "output-path",
            "OUTPUT_PATH_OUTSIDE_ARTIFACT_ROOT",
            f"output must be JSON below {PLAN_ARTIFACT_ROOT}",
        )
        return None
    target = root / PurePosixPath(relative)
    cursor = target.parent
    while cursor != root and cursor != cursor.parent:
        if cursor.is_symlink():
            checks.fail(
                "output-path",
                "OUTPUT_PATH_SYMLINK",
                f"output parent is a symlink: {cursor.name}",
            )
            return None
        cursor = cursor.parent
    if target.is_symlink():
        checks.fail(
            "output-path",
            "OUTPUT_PATH_SYMLINK",
            "output target is a symlink",
        )
        return None
    checks.pass_(
        "output-path", f"ordinary advisory artifact path: {relative}"
    )
    return target
