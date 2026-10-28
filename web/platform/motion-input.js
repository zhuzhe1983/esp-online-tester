// PIR stimulus is distinct from the delayed sensor output read by the guest.
export const motionParameter=parameters=>parameters?.motion===undefined?false:parameters.motion;
