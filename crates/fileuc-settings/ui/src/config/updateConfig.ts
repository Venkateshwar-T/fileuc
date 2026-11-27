export type StandardUiStyle = 'banner' | 'toast' | 'modal';
export type StandardCopyMode = 'simple' | 'value';
export type BlockingUiStyle = 'modal' | 'fullpage';

export interface UpdateUiConfig {
  standardUiStyle: StandardUiStyle;
  standardCopyMode: StandardCopyMode;
  blockingUiStyle: BlockingUiStyle;
  copy: {
    standardHeadline: (version: string) => string;
    standardSimpleBody: string;
    standardValueFallback: string;
    featureModalHeadline: (version: string) => string;
    blockingHeadline: string;
    blockingBody: string;
    downloadingLabel: (version: string) => string;
    updateButtonLabel: string;
    laterButtonLabel: string;
  };
}

/**
 * Modular Update UI & Copy Configuration.
 * Change `standardUiStyle`, `standardCopyMode`, `blockingUiStyle`, or any copy string here
 * without touching updater logic.
 */
export const UPDATE_CONFIG: UpdateUiConfig = {
  standardUiStyle: 'banner',
  standardCopyMode: 'simple',
  blockingUiStyle: 'fullpage',
  copy: {
    standardHeadline: (version: string) => `Update Available · v${version}`,
    standardSimpleBody: 'A new version is available. Update to install.',
    standardValueFallback: 'Update available with performance improvements and bug fixes.',
    featureModalHeadline: (version: string) => `FileUC v${version} is Available`,
    blockingHeadline: 'Update Required',
    blockingBody:
      'To continue using FileUC, please update to the latest version. This update includes essential security and stability improvements.',
    downloadingLabel: (version: string) => `Downloading v${version}...`,
    updateButtonLabel: 'Update',
    laterButtonLabel: 'Remind me later',
  },
};
