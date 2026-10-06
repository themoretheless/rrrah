package io.github.yuvraj0028.bktree;

import java.util.ArrayDeque;
import java.util.ArrayList;
import java.util.Deque;
import java.util.HashMap;
import java.util.List;
import java.util.Map;

import io.github.yuvraj0028.utils.Hamming;

/**
 * Implements a Burkhard-Keller Tree (BK-Tree) for efficient nearest neighbor search
 * in metric spaces. It is particularly effective for large sets of perceptual
 * hashes (like pHash or dHash) where the distance metric is the Hamming distance.
 */
public class BKTree {
    
    // Node class to depict BKTree and its nodes [children]
    private static class Node{
        // The value (e.g., the 64-bit image hash) stored at this node.
        final long value;
        
        // Children are mapped by the distance (Hamming distance) from this node's value.
        // The key 'd' means the child node's value has a distance 'd' from this node's value.
        final Map<Integer, Node> children = new HashMap<>();
        
        Node(long value) {this.value = value;}
    }

    // initial state: the root of the BK-Tree
    private Node root = null;
    
    /**
     * Constructs an empty BKTree.
     */
    public BKTree(){}

    /**
     * Adds an image hash (long value) to the BKTree.
     * The insertion process uses the Hamming distance to determine the path.
     * * @param value The 64-bit hash of the image to add.
     */
    public void add(long value){
        // Case 1: Tree is empty. The new value becomes the root.
        if(root == null){
            root = new Node(value);
            return;
        }

        Node curr = root;
        // Traverse the tree until an insertion point is found.
        while(true){
            // 1. Calculate the Hamming distance between the current node's value and the new value.
            int dist = Hamming.distanceLong(curr.value, value);
            
            // 2. Check if a child node exists for this distance.
            Node child = curr.children.get(dist);
            
            if(child != null){
                // If a child exists at this exact distance, continue the traversal down to the child.
                curr = child;
            } else {
                // If no child exists at this distance, an insertion point is found.
                // 3. Create a new node and add it as a child to the current node,
                //    using the calculated distance as the map key.
                Node newNode = new Node(value);
                curr.children.put(dist, newNode);
                return; // Insertion complete.
            }
        }
    }

    /**
     * Searches for image hashes in the BKTree that are within a specified maximum distance
     * (maxDist) of the query value. This is used to find similar images.
     * * @param value The query hash (image hash) to search for.
     * @param maxDist The maximum allowed Hamming distance for a match (tolerance).
     * @return A list of hashes from the tree that are within maxDist of the query value.
     */
    public List<Long> search(long value, int maxDist){
        List<Long> res = new ArrayList<>();
        if(root == null) return res;

        // Use a Deque (as a stack for DFS or queue for BFS) to manage nodes to visit.
        // A stack is used here, effectively implementing a Depth-First Search (DFS).
        Deque<Node> stack = new ArrayDeque<>();
        stack.push(root);
        
        while(!stack.isEmpty()){
            Node curr = stack.pop();
            
            // 1. Calculate the distance between the current node's value and the query value.
            int dist = Hamming.distanceLong(curr.value, value);
            
            // 2. If the distance is within the tolerance, the current node is a match.
            if(dist <= maxDist){
                res.add(curr.value);
            }
            
            // 3. Pruning Step: Calculate the distance range for relevant children.
            //    According to the triangle inequality theorem (a core BK-Tree principle),
            //    any child node 'C' whose distance from 'curr' is 'd(curr, C)' can only
            //    match the query if 'd(curr, C)' is in the range:
            //    [d(curr, value) - maxDist, d(curr, value) + maxDist]
            int lo = Math.max(0, dist - maxDist); // Lower bound
            int hi = dist + maxDist;              // Upper bound
            
            // 4. Iterate over all children and only push those that fall within the calculated range.
            for (Map.Entry<Integer, Node> e : curr.children.entrySet()) {
                int childDist = e.getKey(); // This is d(curr, child.value)
                
                // If the child's distance from the current node is in the search range,
                // it is possible for the child (or its descendants) to be a match.
                if (childDist >= lo && childDist <= hi) {
                    stack.push(e.getValue());
                }
                // Children outside this range are guaranteed *not* to contain matches
                // within their subtrees, thus pruning the search space.
            }
        }
        return res;
    }

}
