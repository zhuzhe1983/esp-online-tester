// Artifact completeness and verification are independent. A board can try an
// untested image; no UI verdict may substitute for runtime chip/layout checks.
export function firmwareRunPolicy(product, firmware) {
  const boardReady = Boolean(product?.runtime?.board);
  const hasSource = Boolean(firmware?.source?.url || firmware?.source?.manifestUrl);
  const verified = firmware?.simulationVerified === true;
  return {
    canRun: boardReady && hasSource,
    trial: hasSource && !verified,
    label: !hasSource ? '资料入口' : verified ? '已验证操作' : '可尝试运行',
    reason: !boardReady ? 'board-unavailable' : !hasSource ? 'artifact-missing' : null,
  };
}
