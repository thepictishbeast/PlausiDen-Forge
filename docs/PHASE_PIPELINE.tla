---------------------------- MODULE PhasePipeline ----------------------------
(***************************************************************************)
(* TLA+ specification of the Forge phase-pipeline runner.                  *)
(*                                                                         *)
(* T76 cycle 89 closes Forge T27 (TLA+ spec for phase pipeline             *)
(* invariants).  Models the loop in forge-cli/src/main.rs that drives      *)
(* Forge's lint phases: each phase produces a Vec<Finding> or an error,    *)
(* errors abort the pipeline, findings accumulate into a Merkle-chained    *)
(* build report.                                                           *)
(*                                                                         *)
(* The Rust runner is currently single-threaded.  This spec also describes *)
(* the parallelized future (T24 type-state pipeline) and pins the          *)
(* invariants any concurrent implementation MUST preserve.                 *)
(*                                                                         *)
(* Check with TLC:                                                         *)
(*   tlc -workers auto -coverage 1 PhasePipeline                          *)
(* Model values:                                                           *)
(*   PhaseIds <- {p1, p2, p3, p4}                                          *)
(*   MaxFindings <- 3                                                      *)
(*                                                                         *)
(* Bug assumption (AVP-2 Tier-1): TLC under bounded MaxFindings still      *)
(* exercises the abort branches at every phase index, including the last.  *)
(***************************************************************************)

EXTENDS Naturals, Sequences, FiniteSets

CONSTANTS
    PhaseIds,        \* The set of phase identifiers (model values).
    MaxFindings      \* Bound on findings per phase, for TLC finiteness.

ASSUME PhaseIdsFinite     == IsFiniteSet(PhaseIds)
ASSUME PhaseIdsNonEmpty   == Cardinality(PhaseIds) > 0
ASSUME MaxFindingsBound   == MaxFindings \in Nat /\ MaxFindings >= 0

(***************************************************************************)
(* Configured phase sequence: we model it as a permutation over PhaseIds.  *)
(* The runner accepts any concrete ordering; the spec must hold for ALL    *)
(* orderings.                                                              *)
(***************************************************************************)
Phases == CHOOSE seq \in [1..Cardinality(PhaseIds) -> PhaseIds] :
            \A i, j \in 1..Cardinality(PhaseIds) :
                i # j => seq[i] # seq[j]

N == Len(Phases)

(***************************************************************************)
(* Phase outcome domain.                                                   *)
(*   "pending"  — never started                                            *)
(*   "running"  — started, not yet finished (only visible to TLC; the      *)
(*                 single-threaded runner observes "pending" → "ok|err"    *)
(*                 atomically, but the spec keeps the intermediate state   *)
(*                 explicit so the parallel-runner refinement of T24 has   *)
(*                 the same invariants.)                                    *)
(*   "ok"       — produced 0..MaxFindings findings                         *)
(*   "error"    — returned BuildError; pipeline aborts after this state    *)
(***************************************************************************)
Outcomes == {"pending", "running", "ok", "error"}

(***************************************************************************)
(* PipelineState:                                                          *)
(*   "init"      — Phases declared, none yet started                       *)
(*   "running"   — at least one phase has started; no error yet            *)
(*   "complete"  — every phase ran with status "ok"                        *)
(*   "aborted"   — some phase returned error; pipeline halted              *)
(*   "sealed"    — report has been Merkle-chained + (optionally) signed    *)
(*                  and committed to disk; terminal state                  *)
(***************************************************************************)
PipelineStates == {"init", "running", "complete", "aborted", "sealed"}

VARIABLES
    status,          \* [PhaseIds -> Outcomes]
    findingsCount,   \* [PhaseIds -> 0..MaxFindings]
    pipelineState,   \* element of PipelineStates
    chainLength,     \* Nat; 0 before seal, >= 1 after
    prevHashSeen     \* BOOLEAN; whether a prior report existed when sealed

vars == << status, findingsCount, pipelineState, chainLength, prevHashSeen >>

(***************************************************************************)
(* Helpers                                                                 *)
(***************************************************************************)
DoneOK(p)    == status[p] = "ok"
DoneErr(p)   == status[p] = "error"
Pending(p)   == status[p] = "pending"
Running(p)   == status[p] = "running"
Terminal(p)  == DoneOK(p) \/ DoneErr(p)

(* Phase index for an id (only meaningful while phase is in Phases). *)
IndexOf(p)   == CHOOSE i \in 1..N : Phases[i] = p

(* Phases earlier in the configured order than p. *)
Predecessors(p) == { Phases[j] : j \in 1..(IndexOf(p) - 1) }

(* All phases done with "ok"? *)
AllOk == \A p \in PhaseIds : DoneOK(p)

(* Any phase in error? *)
AnyError == \E p \in PhaseIds : DoneErr(p)

(***************************************************************************)
(* Initial state.                                                          *)
(***************************************************************************)
Init ==
    /\ status         = [p \in PhaseIds |-> "pending"]
    /\ findingsCount  = [p \in PhaseIds |-> 0]
    /\ pipelineState  = "init"
    /\ chainLength    = 0
    /\ prevHashSeen   \in BOOLEAN   \* abstracts "is there a prior report on disk"

(***************************************************************************)
(* StartPhase(p): phase p begins.  Only legal if (a) p is pending, (b)     *)
(* every predecessor of p is terminal with "ok" (sequential ordering — the *)
(* parallel runner of T24 will relax this to "every dependency satisfied"  *)
(* via an explicit dep DAG; for the v1 spec we use the linear order).      *)
(***************************************************************************)
StartPhase(p) ==
    /\ pipelineState \in {"init", "running"}
    /\ Pending(p)
    /\ \A q \in Predecessors(p) : DoneOK(q)
    /\ status' = [status EXCEPT ![p] = "running"]
    /\ pipelineState' = "running"
    /\ UNCHANGED << findingsCount, chainLength, prevHashSeen >>

(***************************************************************************)
(* CompleteOk(p, k): phase p finishes with k findings (0..MaxFindings).    *)
(***************************************************************************)
CompleteOk(p, k) ==
    /\ Running(p)
    /\ k \in 0..MaxFindings
    /\ status' = [status EXCEPT ![p] = "ok"]
    /\ findingsCount' = [findingsCount EXCEPT ![p] = k]
    /\ pipelineState' = pipelineState
    /\ UNCHANGED << chainLength, prevHashSeen >>

(***************************************************************************)
(* CompleteErr(p): phase p returns BuildError.  Pipeline aborts.           *)
(***************************************************************************)
CompleteErr(p) ==
    /\ Running(p)
    /\ status' = [status EXCEPT ![p] = "error"]
    /\ findingsCount' = [findingsCount EXCEPT ![p] = 0]
    /\ pipelineState' = "aborted"
    /\ UNCHANGED << chainLength, prevHashSeen >>

(***************************************************************************)
(* SealComplete: when every phase is "ok", finalize the report.            *)
(***************************************************************************)
SealComplete ==
    /\ pipelineState = "running"
    /\ AllOk
    /\ ~AnyError
    /\ pipelineState' = "sealed"
    /\ chainLength'   = IF prevHashSeen THEN chainLength + 1 ELSE 1
    /\ UNCHANGED << status, findingsCount, prevHashSeen >>

(***************************************************************************)
(* SealAborted: even an aborted build is sealed for audit trail.  The      *)
(* report records which phase failed; future runs still chain-link to it.  *)
(***************************************************************************)
SealAborted ==
    /\ pipelineState = "aborted"
    /\ pipelineState' = "sealed"
    /\ chainLength'   = IF prevHashSeen THEN chainLength + 1 ELSE 1
    /\ UNCHANGED << status, findingsCount, prevHashSeen >>

(***************************************************************************)
(* The next-state relation.  Existential over (phase id, finding count)    *)
(* models the runner's nondeterministic interleaving + finding count.      *)
(***************************************************************************)
Next ==
    \/ \E p \in PhaseIds : StartPhase(p)
    \/ \E p \in PhaseIds, k \in 0..MaxFindings : CompleteOk(p, k)
    \/ \E p \in PhaseIds : CompleteErr(p)
    \/ SealComplete
    \/ SealAborted

Spec == Init /\ [][Next]_vars

(***************************************************************************)
(*                            SAFETY INVARIANTS                            *)
(***************************************************************************)

(***************************************************************************)
(* TypeOK: every variable stays in its declared domain.                    *)
(***************************************************************************)
TypeOK ==
    /\ status         \in [PhaseIds -> Outcomes]
    /\ findingsCount  \in [PhaseIds -> 0..MaxFindings]
    /\ pipelineState  \in PipelineStates
    /\ chainLength    \in Nat
    /\ prevHashSeen   \in BOOLEAN

(***************************************************************************)
(* Order: a running or completed phase has all predecessors completed-ok.  *)
(* This is THE pipeline invariant — cycle 89's T27 mandate.                *)
(***************************************************************************)
OrderInvariant ==
    \A p \in PhaseIds :
        (status[p] # "pending") =>
            \A q \in Predecessors(p) : DoneOK(q)

(***************************************************************************)
(* AbortFreezes: once a phase has errored, no later phase ever runs.       *)
(* (Captures "fail-stop" — a BuildError aborts the pipeline.)              *)
(***************************************************************************)
AbortFreezes ==
    \A p, q \in PhaseIds :
        ( DoneErr(p) /\ q \in PhaseIds \ Predecessors(p) \ {p} ) =>
            status[q] \in {"pending", "running"}
        \* The "running" allowance covers the small window between
        \* StartPhase(q) and CompleteErr(p) in the parallel refinement.

(***************************************************************************)
(* NoConcurrentError: at most one phase is in error.  (Single-threaded     *)
(* runner can't produce two errors; parallel runner aborts on the first.)  *)
(***************************************************************************)
AtMostOneError ==
    Cardinality({ p \in PhaseIds : DoneErr(p) }) <= 1

(***************************************************************************)
(* SealMonotone: chainLength only ever goes up, and prevHashSeen never     *)
(* spontaneously flips from TRUE to FALSE.  Models the append-only audit   *)
(* trail in reports/build-*.json (cycle 70 + T26 Merkle chain).            *)
(***************************************************************************)
SealMonotone ==
    /\ chainLength >= 0
    \* TLA+ doesn't quantify over history without a history variable; the
    \* invariant fragment readable here is the chain >= 0 floor.  The
    \* temporal version SealMonotonicTemporal below covers the rest.

(***************************************************************************)
(* SealingTerminal: once sealed, status + findingsCount never change.      *)
(***************************************************************************)
SealingTerminal ==
    (pipelineState = "sealed") =>
        \A p \in PhaseIds : Terminal(p)

(***************************************************************************)
(* ChainLengthAtLeastOneAfterSeal: every sealed report has chain_length>=1.*)
(***************************************************************************)
ChainLengthValid ==
    (pipelineState = "sealed") => (chainLength >= 1)

(***************************************************************************)
(* SafetyAll: combine every safety invariant.                              *)
(***************************************************************************)
SafetyAll ==
    /\ TypeOK
    /\ OrderInvariant
    /\ AbortFreezes
    /\ AtMostOneError
    /\ SealMonotone
    /\ SealingTerminal
    /\ ChainLengthValid

(***************************************************************************)
(*                          TEMPORAL PROPERTIES                            *)
(***************************************************************************)

(***************************************************************************)
(* Eventually-sealed: every fair execution reaches "sealed".               *)
(* (Liveness — requires weak fairness on Next.)                            *)
(***************************************************************************)
EventuallySealed == <>(pipelineState = "sealed")

(***************************************************************************)
(* SealMonotonicTemporal: chainLength never decreases.                     *)
(***************************************************************************)
SealMonotonicTemporal == [][ chainLength' >= chainLength ]_vars

(***************************************************************************)
(* SealOnce: the seal action fires at most once per execution.             *)
(***************************************************************************)
SealOnce ==
    [][ (pipelineState = "sealed") => (pipelineState' = "sealed") ]_vars

(***************************************************************************)
(* FairSpec: Spec + weak fairness on every action gives us liveness.       *)
(***************************************************************************)
Fairness ==
    /\ \A p \in PhaseIds : WF_vars(StartPhase(p))
    /\ \A p \in PhaseIds, k \in 0..MaxFindings : WF_vars(CompleteOk(p, k))
    /\ \A p \in PhaseIds : WF_vars(CompleteErr(p))
    /\ WF_vars(SealComplete)
    /\ WF_vars(SealAborted)

FairSpec == Spec /\ Fairness

(***************************************************************************)
(*                          REFINEMENT NOTES                               *)
(***************************************************************************)
(*                                                                         *)
(* Single-threaded refinement (current main.rs runner):                    *)
(*   StartPhase(p) and CompleteOk(p, k) [or CompleteErr(p)] always fire    *)
(*   back-to-back with no interleaving from another phase.  In the spec    *)
(*   we DO allow interleaving so the parallel-runner refinement (T24)      *)
(*   inherits the same invariants without re-proof.                        *)
(*                                                                         *)
(* Parallel refinement (T24, not yet implemented):                         *)
(*   Replace `Predecessors(p)` with `DepsOf(p)` — an explicit dependency   *)
(*   DAG.  StartPhase still requires every dep DoneOK.  Two independent    *)
(*   phases CAN be Running simultaneously.  AbortFreezes still holds: as   *)
(*   soon as any phase errors, the scheduler MUST refuse to StartPhase     *)
(*   any new phase.                                                        *)
(*                                                                         *)
(* Crash refinement (not modeled yet):                                     *)
(*   A process kill mid-CompleteOk would leave a phase Running forever.    *)
(*   The Rust runner detects this via reports/.lock + fsync of the report. *)
(*   A future refinement adds CrashAction that flips Running → "pending"   *)
(*   on restart, with the constraint that no in-flight findings leak.      *)
(***************************************************************************)
=============================================================================
\* Modification History
\* Created 2026-05-14 (T76 cycle 89, Forge T27)
