import { configure } from '@testing-library/react';

// waitFor and findBy* default to 1 s, too short under parallel load.
configure({ asyncUtilTimeout: 5_000 });
